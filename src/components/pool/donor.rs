//! The donor side: when this machine may claim a job (pure rules), the supervised run of a template
//! (plain child processes, killed with the app), and the protocol with kun (claim, progress, done,
//! upload, finish). Nothing here runs a shell.

// Progress fractions and byte counts: the precision lost is far below what a progress bar shows.
#![allow(clippy::cast_precision_loss)]

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};

use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
};

use super::{
    model::{Claimed, DoneAnswer, FileSpec, PoolFile, PoolJob, ProgressAnswer},
    params::{self, Cmd, MAX_LOOSE_FRAMES, Params, RenderFormat, TextFormat},
};
use crate::{
    network::api::{self, Api, ApiError},
    settings::ShareSettings,
    tasks::upload::{self, UploadTarget},
};

/// A person who touched the machine this recently is "back": the job stops.
pub const ACTIVE_BELOW_SECS: f64 = 60.0;
const WATCH_EVERY: Duration = Duration::from_secs(5);
const LOG_KEEP: usize = 3800;
/// Watcher ticks between progress posts: 3 x 5 s = every 15 s.
const PROGRESS_TICKS: u32 = 3;

// ---- what is installed here ----------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tools {
    pub whisper: bool,
    pub ffmpeg: bool,
    pub blender: bool,
}

/// Kinds whose tools are installed (transcribe needs whisper and ffmpeg for the wav).
#[must_use]
pub fn kinds_available(t: &Tools) -> Vec<String> {
    let mut out = Vec::new();
    if t.whisper && t.ffmpeg {
        out.push("transcribe".to_owned());
    }
    if t.blender {
        out.push("render".to_owned());
    }
    if t.ffmpeg {
        out.push("encode".to_owned());
    }
    out
}

/// What the rules look at: the opt-in and the machine right now. `None` means the reading failed,
/// and an unknown power or idle reading never counts as eligible.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub share: ShareSettings,
    pub on_ac: Option<bool>,
    pub idle_secs: Option<f64>,
    pub ram_free: Option<f64>,
    pub disk_free: Option<f64>,
    pub has_gpu: Option<bool>,
    pub tools: Tools,
    /// The whisper model files found (`ggml-base.bin`, …).
    pub models: Vec<String>,
}

/// Why the machine may not claim anything now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Off,
    NoKinds,
    Battery,
    Active,
    Unknown,
}

impl Why {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::NoKinds => "no-kinds",
            Self::Battery => "battery",
            Self::Active => "active",
            Self::Unknown => "unknown",
        }
    }
}

/// The machine-level gate: opted in, on mains, and idle for `idleMinutes`.
///
/// # Errors
/// The first rule that fails.
pub fn gate(s: &Snapshot) -> Result<(), Why> {
    if !s.share.enabled {
        return Err(Why::Off);
    }
    if s.share.kinds.is_empty() {
        return Err(Why::NoKinds);
    }
    match s.on_ac {
        None => return Err(Why::Unknown),
        Some(false) => return Err(Why::Battery),
        Some(true) => {}
    }
    let idle = s.idle_secs.ok_or(Why::Unknown)?;
    if idle < f64::from(s.share.idle_minutes) * 60.0 {
        return Err(Why::Active);
    }
    Ok(())
}

/// Whether one queued job fits this machine: a kind it accepts and has the tool for, valid
/// parameters, a model for transcribe, and RAM, disk and GPU for its needs.
///
/// # Errors
/// A short reason, for the log.
pub fn job_fits(job: &PoolJob, s: &Snapshot) -> Result<(), String> {
    if job.status != "queued" {
        return Err("not queued".into());
    }
    if !s.share.kinds.contains(&job.kind) || !kinds_available(&s.tools).contains(&job.kind) {
        return Err("kind not accepted here".into());
    }
    let p = params::validate(&job.kind, &job.params)?;
    match &p {
        Params::Transcribe { model, .. } => {
            let have = match model {
                Some(m) => s.models.contains(&format!("ggml-{m}.bin")),
                None => !s.models.is_empty(),
            };
            if !have {
                return Err("no whisper model for this job".into());
            }
        }
        Params::Render { format: RenderFormat::Mp4, .. } if !s.tools.ffmpeg => return Err("ffmpeg is needed for mp4".into()),
        _ => {}
    }
    let inputs: f64 = job.inputs.iter().map(|f| to_f64(f.size)).sum();
    let needs = job.needs.clone().unwrap_or_default();
    // With no stated need, room for the inputs, the work copy and the outputs.
    let disk = needs.disk_bytes.unwrap_or(inputs * 3.0);
    if s.disk_free.is_some_and(|free| free < disk) {
        return Err("not enough disk".into());
    }
    if let (Some(ram), Some(free)) = (needs.ram_bytes, s.ram_free)
        && free < ram
    {
        return Err("not enough memory".into());
    }
    if needs.gpu == Some(true) && s.has_gpu == Some(false) {
        return Err("a GPU is needed".into());
    }
    Ok(())
}

#[allow(clippy::cast_precision_loss)]
fn to_f64(n: u64) -> f64 {
    n as f64
}

// ---- finding the whisper model -------------------------------------------------------------------

/// The ggml files under `dirs` (three levels deep), at least 10 MB and not test stubs.
#[must_use]
pub fn find_models(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in dirs {
        for e in walkdir::WalkDir::new(d).max_depth(3).into_iter().filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let bin = Path::new(&name).extension().is_some_and(|x| x == "bin");
            if name.starts_with("ggml-") && bin && !name.contains("test") && e.metadata().is_ok_and(|m| m.is_file() && m.len() > 10_000_000) {
                out.push(e.path().to_path_buf());
            }
        }
    }
    out.sort();
    out
}

/// Where the whisper models usually are (the readiness probe looks in the same places).
#[must_use]
pub fn model_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = dirs::home_dir().map(|h| vec![h.join(".cache/whisper"), h.join("models"), h.join(".local/share/whisper"), h.join(".kun/machine/models")]).unwrap_or_default();
    dirs.extend(["/opt/homebrew/share/whisper-cpp", "/usr/local/share/whisper-cpp"].map(PathBuf::from));
    dirs
}

/// The file for the requested model (`base` → `ggml-base.bin`), or the first one when none was named.
#[must_use]
pub fn pick_model(found: &[PathBuf], model: Option<&str>) -> Option<PathBuf> {
    match model {
        Some(m) => {
            let want = format!("ggml-{m}.bin");
            found.iter().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&want))).cloned()
        }
        None => found.first().cloned(),
    }
}

// ---- running a child -----------------------------------------------------------------------------

/// Lowers a command's priority with a fixed prefix: `nice -n 10` on unix. (Windows sets a priority
/// class on the process instead, in [`exec`].)
#[must_use]
pub fn lowered(cmd: &Cmd) -> Cmd {
    if cfg!(unix) {
        let mut args = vec!["-n".to_owned(), "10".to_owned(), cmd.program.clone()];
        args.extend(cmd.args.iter().cloned());
        Cmd { program: "nice".into(), args }
    } else {
        cmd.clone()
    }
}

/// Why a run stopped before it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abort {
    /// kun said the job is no longer ours: stop and clean up.
    Server,
    /// The person came back: stop and release the job.
    Active,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    Aborted(Abort),
    Failed(String),
    /// An input link answered 403/404: release the job instead of failing it.
    InputMissing,
}

/// Shared between the run and its watcher.
#[derive(Default)]
pub struct Ctl {
    abort: AtomicU8,
    pub finished: AtomicBool,
    pub progress: Mutex<f64>,
    pub log: Mutex<String>,
}

impl Ctl {
    pub fn abort(&self, why: Abort) {
        let _ = self.abort.compare_exchange(0, if why == Abort::Server { 1 } else { 2 }, Ordering::SeqCst, Ordering::SeqCst);
    }

    #[must_use]
    pub fn aborted(&self) -> Option<Abort> {
        match self.abort.load(Ordering::SeqCst) {
            1 => Some(Abort::Server),
            2 => Some(Abort::Active),
            _ => None,
        }
    }

    pub fn set_progress(&self, p: f64) {
        *self.progress.lock().unwrap_or_else(PoisonError::into_inner) = p.clamp(0.0, 1.0);
    }

    #[must_use]
    pub fn progress(&self) -> f64 {
        *self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn note(&self, line: &str) {
        let mut log = self.log.lock().unwrap_or_else(PoisonError::into_inner);
        log.push_str(&line.chars().take(300).collect::<String>());
        log.push('\n');
        if log.len() > LOG_KEEP * 2 {
            let cut = log.len() - LOG_KEEP;
            let at = (cut..log.len()).find(|i| log.is_char_boundary(*i)).unwrap_or(0);
            *log = log[at..].to_string();
        }
    }

    #[must_use]
    pub fn log_tail(&self) -> String {
        let log = self.log.lock().unwrap_or_else(PoisonError::into_inner);
        let start = log.len().saturating_sub(LOG_KEEP);
        let at = (start..=log.len()).find(|i| log.is_char_boundary(*i)).unwrap_or(0);
        log[at..].to_owned()
    }
}

/// Forwards a pipe's lines. `\r` ends a line too: whisper and ffmpeg redraw progress in place.
async fn pump(pipe: impl tokio::io::AsyncRead + Unpin, tx: mpsc::UnboundedSender<String>) {
    let mut r = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                for part in String::from_utf8_lossy(&buf).split(['\n', '\r']).filter(|p| !p.trim().is_empty()) {
                    if tx.send(part.to_owned()).is_err() {
                        return;
                    }
                }
            }
        }
    }
}

/// Runs one command to its end at low priority, feeding each output line to `on_line`. A cancel
/// (see [`Ctl::abort`]) kills the child. Dropped futures kill it too (`kill_on_drop`).
///
/// # Errors
/// [`RunError::Aborted`] on a cancel, [`RunError::Failed`] when it cannot start or exits non-zero.
pub async fn exec(cmd: &Cmd, ctl: &Ctl, mut on_line: impl FnMut(&str) + Send) -> Result<(), RunError> {
    let low = lowered(cmd);
    let mut c = Command::new(&low.program);
    c.args(&low.args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(0x0000_4000 | 0x0800_0000); // BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW
    let mut child = c.spawn().map_err(|e| RunError::Failed(format!("could not start {}: {e}", cmd.program)))?;
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    if let Some(r) = child.stdout.take() {
        tokio::spawn(pump(r, tx.clone()));
    }
    if let Some(r) = child.stderr.take() {
        tokio::spawn(pump(r, tx.clone()));
    }
    drop(tx);
    loop {
        tokio::select! {
            line = rx.recv() => match line {
                Some(l) => on_line(&l),
                None => break,
            },
            () = tokio::time::sleep(Duration::from_millis(250)) => {
                if let Some(a) = ctl.aborted() {
                    let _ = child.kill().await;
                    return Err(RunError::Aborted(a));
                }
            }
        }
    }
    let status = child.wait().await.map_err(|e| RunError::Failed(e.to_string()))?;
    if let Some(a) = ctl.aborted() {
        return Err(RunError::Aborted(a));
    }
    if status.success() { Ok(()) } else { Err(RunError::Failed(format!("{} exited with {status}", cmd.program))) }
}

/// A command's full output, for the one-shot reads (`ffprobe`, `blender --version`).
pub async fn capture(cmd: &Cmd, ctl: &Ctl) -> Option<String> {
    let mut text = String::new();
    exec(cmd, ctl, |l| {
        text.push_str(l);
        text.push('\n');
    })
    .await
    .ok()?;
    Some(text)
}

// ---- the templates -------------------------------------------------------------------------------

/// The programs a run needs.
#[derive(Debug, Clone, Default)]
pub struct ToolPaths {
    pub whisper: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub blender: Option<PathBuf>,
}

fn need(p: Option<&PathBuf>, name: &str) -> Result<PathBuf, RunError> {
    p.cloned().ok_or_else(|| RunError::Failed(format!("{name} is not installed")))
}

fn ffprobe_of(ffmpeg: &Path) -> PathBuf {
    ffmpeg.with_file_name(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" })
}

/// Runs the job's template over the downloaded `inputs` and returns the output files (in `out`).
///
/// # Errors
/// [`RunError`]: a cancel, a failed tool, or no output.
pub async fn run_template(p: &Params, inputs: &[PathBuf], work: &Path, out: &Path, tools: &ToolPaths, ctl: &Ctl) -> Result<Vec<PathBuf>, RunError> {
    let io = |e: std::io::Error| RunError::Failed(e.to_string());
    std::fs::create_dir_all(out).map_err(io)?;
    let n = inputs.len().max(1);
    let mut outputs = Vec::new();
    match p {
        Params::Transcribe { model, language, format } => {
            let (whisper, ffmpeg) = (need(tools.whisper.as_ref(), "whisper-cli")?, need(tools.ffmpeg.as_ref(), "ffmpeg")?);
            let found = find_models(&model_dirs());
            let model_path = pick_model(&found, model.as_deref()).ok_or_else(|| RunError::Failed("the whisper model is not installed".into()))?;
            for (i, input) in inputs.iter().enumerate() {
                let base = i as f64 / n as f64;
                let wav = work.join(format!("{i}.wav"));
                exec(&params::wav_cmd(&ffmpeg, input, &wav), ctl, |l| ctl.note(l)).await?;
                let name = input.file_name().map(|f| params::stem(&f.to_string_lossy())).unwrap_or_default();
                let prefix = out.join(&name);
                let step = 1.0 / n as f64;
                exec(&params::whisper_cmd(&whisper, &model_path, &wav, &prefix, *format, language.as_deref()), ctl, |l| {
                    ctl.note(l);
                    if let Some(pr) = params::whisper_progress(l) {
                        ctl.set_progress(base + pr * step);
                    }
                })
                .await?;
                outputs.push(out.join(format!("{name}.{}", if *format == TextFormat::Srt { "srt" } else { "txt" })));
                ctl.set_progress((i + 1) as f64 / n as f64);
            }
        }
        Params::Encode { preset } => {
            let ffmpeg = need(tools.ffmpeg.as_ref(), "ffmpeg")?;
            let probe = ffprobe_of(&ffmpeg);
            for (i, input) in inputs.iter().enumerate() {
                let base = i as f64 / n as f64;
                let step = 1.0 / n as f64;
                let duration = capture(&Cmd { program: probe.to_string_lossy().into_owned(), args: ["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"].iter().map(|s| (*s).to_owned()).chain([input.to_string_lossy().into_owned()]).collect() }, ctl).await.and_then(|o| params::parse_duration(&o));
                let name = input.file_name().map(|f| params::stem(&f.to_string_lossy())).unwrap_or_default();
                let dest = out.join(format!("{name}-{}.mp4", preset.id()));
                exec(&params::encode_cmd(&ffmpeg, *preset, cfg!(target_os = "macos"), input, &dest), ctl, |l| {
                    if let (Some(t), Some(d)) = (params::ffmpeg_out_time(l), duration) {
                        ctl.set_progress(base + (t / d).min(1.0) * step);
                    } else if !l.starts_with(['o', 'f', 'b', 't', 'd', 'p', 's']) || l.contains("rror") {
                        ctl.note(l);
                    }
                })
                .await?;
                outputs.push(dest);
                ctl.set_progress((i + 1) as f64 / n as f64);
            }
        }
        Params::Render { engine, start, end, format } => {
            let blender = need(tools.blender.as_ref(), "blender")?;
            let file = inputs.iter().find(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("blend"))).or(inputs.first()).ok_or_else(|| RunError::Failed("no .blend file".into()))?;
            let version = capture(&Cmd { program: blender.to_string_lossy().into_owned(), args: vec!["--version".into()] }, ctl).await.and_then(|o| params::parse_blender_version(&o)).unwrap_or((4, 2));
            let engine_id = params::engine_id(*engine, version);
            let frames = work.join("frames");
            std::fs::create_dir_all(&frames).map_err(io)?;
            let total = f64::from(end - start + 1);
            exec(&params::blender_render_cmd(&blender, engine_id, file, &frames, *start, *end), ctl, |l| {
                ctl.note(l);
                if let Some(f) = params::blender_frame(l) {
                    ctl.set_progress((f64::from(f.saturating_sub(*start)) + 1.0) / total * 0.95);
                }
            })
            .await?;
            let mut pngs: Vec<PathBuf> = std::fs::read_dir(&frames).map_err(io)?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "png")).collect();
            pngs.sort();
            if pngs.is_empty() {
                return Err(RunError::Failed("blender wrote no frames".into()));
            }
            let stem = params::stem(&file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default());
            if *format == RenderFormat::Mp4 {
                let ffmpeg = need(tools.ffmpeg.as_ref(), "ffmpeg")?;
                let fps = capture(&params::blender_fps_cmd(&blender, file), ctl).await.and_then(|o| params::parse_kun_fps(&o)).unwrap_or(24);
                let dest = out.join(format!("{stem}.mp4"));
                exec(&params::frames_to_mp4_cmd(&ffmpeg, &frames, *start, fps, &dest), ctl, |l| ctl.note(l)).await?;
                outputs.push(dest);
            } else if pngs.len() <= MAX_LOOSE_FRAMES {
                outputs = pngs;
            } else {
                let archive = out.join("frames.tar.gz");
                exec(&params::tar_cmd(&archive, &frames), ctl, |l| ctl.note(l)).await?;
                outputs.push(archive);
            }
        }
    }
    if outputs.iter().any(|o| !o.is_file()) {
        return Err(RunError::Failed("the tool finished without writing its output".into()));
    }
    ctl.set_progress(1.0);
    Ok(outputs)
}

// ---- the protocol --------------------------------------------------------------------------------

/// Why a claim did not give a job.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// kun's reason (`taken`, `kind`, `target`, `share`, `needs`, `own`, `not-found`, `doctor`).
    Reason(String),
    Api(String),
}

/// Takes a job. kun answers 409 `{ ok: false, error }` for a refusal.
///
/// # Errors
/// [`Refused`] with kun's reason, or the transport failure.
pub async fn claim(api: &impl Api, id: &str) -> Result<Claimed, Refused> {
    match api::post_json::<Claimed>(api, "/api/machine/pool/claim", &json!({ "id": id })).await {
        Ok(c) => Ok(c),
        Err(ApiError::Status { status: 409, body }) => {
            let reason = serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v.get("error")?.as_str().map(str::to_owned));
            Err(Refused::Reason(reason.unwrap_or_else(|| "refused".into())))
        }
        Err(e) => Err(Refused::Api(e.to_string())),
    }
}

/// Posts progress; `true` when kun says the job is no longer ours.
pub async fn report(api: &impl Api, id: &str, progress: f64, log: &str) -> bool {
    api::post_json::<ProgressAnswer>(api, "/api/machine/pool/progress", &json!({ "id": id, "progress": progress, "log": log })).await.is_ok_and(|a| a.cancelled)
}

/// Gives the job back to the queue (the person came back, or an input is missing).
pub async fn release(api: &impl Api, id: &str) {
    let _ = api::post_json::<serde_json::Value>(api, "/api/machine/pool/cancel", &json!({ "id": id })).await;
}

/// The content type an output is stored with.
#[must_use]
pub fn output_type(name: &str) -> &'static str {
    if name.ends_with(".tar.gz") { "application/gzip" } else { upload::content_type_of(name) }
}

fn link_ok(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://127.0.0.1") || u.starts_with("http://localhost")
}

/// Streams a presigned link to a file.
///
/// # Errors
/// [`RunError::InputMissing`] on 403/404, else [`RunError::Failed`].
pub async fn download(client: &reqwest::Client, url: &str, dest: &Path) -> Result<(), RunError> {
    if !link_ok(url) {
        return Err(RunError::Failed("an input link is not https".into()));
    }
    let mut res = client.get(url).send().await.map_err(|e| RunError::Failed(format!("download failed: {}", e.without_url())))?;
    let status = res.status();
    if matches!(status.as_u16(), 403 | 404) {
        return Err(RunError::InputMissing);
    }
    if !status.is_success() {
        return Err(RunError::Failed(format!("download refused ({})", status.as_u16())));
    }
    let mut file = tokio::fs::File::create(dest).await.map_err(|e| RunError::Failed(e.to_string()))?;
    while let Some(chunk) = res.chunk().await.map_err(|e| RunError::Failed(format!("download failed: {}", e.without_url())))? {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await.map_err(|e| RunError::Failed(e.to_string()))?;
    }
    Ok(())
}

/// Local file names for the inputs: safe, and distinct.
#[must_use]
pub fn local_names(inputs: &[PoolFile]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    inputs
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut n = params::safe_name(&f.name);
            if seen.contains(&n) {
                n = format!("{i}-{n}");
            }
            seen.push(n.clone());
            n
        })
        .collect()
}

/// Deletes the job's folder when dropped, so every exit path cleans up.
pub struct Cleanup(pub PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The folder the pool works in.
#[must_use]
pub fn pool_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Kun/pool"))
}

/// A job id is safe to use as a folder name.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// What happened to a claimed job.
#[derive(Debug, PartialEq, Eq)]
pub enum Finish {
    Done,
    Failed(String),
    Released,
    Stopped,
}

/// The whole life of a claimed job: download, run under watch, upload, finish. Always cleans up.
pub async fn run_claimed(api: &impl Api, client: &reqwest::Client, claimed: Claimed, tools: &ToolPaths, ctl: Arc<Ctl>, check_idle: impl Fn() -> Option<f64> + Send + Sync) -> Finish {
    let job = claimed.job;
    let Some(root) = pool_root().filter(|_| valid_id(&job.id)) else {
        release(api, &job.id).await;
        return Finish::Released;
    };
    let dir = root.join(&job.id);
    let _clean = Cleanup(dir.clone());
    let _ = std::fs::remove_dir_all(&dir);
    let (inbox, work, out) = (dir.join("in"), dir.join("work"), dir.join("out"));
    if [&inbox, &work, &out].iter().any(|d| std::fs::create_dir_all(d).is_err()) {
        release(api, &job.id).await;
        return Finish::Released;
    }
    let params = match params::validate(&job.kind, &job.params) {
        Ok(p) => p,
        Err(e) => return fail(api, &job.id, &e).await,
    };
    let names = local_names(&claimed.inputs);
    let mut files = Vec::new();
    for (f, name) in claimed.inputs.iter().zip(&names) {
        let (Some(url), dest) = (f.get_url.as_deref(), inbox.join(name)) else {
            return fail(api, &job.id, "an input has no link").await;
        };
        match download(client, url, &dest).await {
            Ok(()) => files.push(dest),
            Err(RunError::InputMissing) => {
                release(api, &job.id).await;
                return Finish::Released;
            }
            Err(e) => return fail(api, &job.id, &format!("{e:?}")).await,
        }
    }
    let run = async {
        let r = run_template(&params, &files, &work, &out, tools, &ctl).await;
        ctl.finished.store(true, Ordering::SeqCst);
        r
    };
    let watch = async {
        let mut tick = 0u32;
        while !ctl.finished.load(Ordering::SeqCst) {
            tokio::time::sleep(WATCH_EVERY).await;
            if ctl.finished.load(Ordering::SeqCst) {
                break;
            }
            if check_idle().is_some_and(|i| i < ACTIVE_BELOW_SECS) {
                ctl.abort(Abort::Active);
            }
            tick += 1;
            if tick.is_multiple_of(PROGRESS_TICKS) && report(api, &job.id, ctl.progress(), &ctl.log_tail()).await {
                ctl.abort(Abort::Server);
            }
        }
    };
    let (result, ()) = tokio::join!(run, watch);
    match result {
        Err(RunError::Aborted(Abort::Server)) => Finish::Stopped,
        Err(RunError::Aborted(Abort::Active) | RunError::InputMissing) => {
            release(api, &job.id).await;
            Finish::Released
        }
        Err(RunError::Failed(e)) => fail(api, &job.id, &e).await,
        Ok(outputs) => deliver(api, client, &job.id, &outputs).await,
    }
}

async fn fail(api: &impl Api, id: &str, error: &str) -> Finish {
    let msg: String = error.chars().take(900).collect();
    let _ = api::post_json::<serde_json::Value>(api, "/api/machine/pool/done", &json!({ "id": id, "ok": false, "error": msg })).await;
    Finish::Failed(msg)
}

/// done → PUT each output → finish. A failed upload releases the job (kun re-queues it).
async fn deliver(api: &impl Api, client: &reqwest::Client, id: &str, outputs: &[PathBuf]) -> Finish {
    let mut specs = Vec::new();
    for o in outputs {
        let name = o.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let size = std::fs::metadata(o).map_or(0, |m| m.len());
        specs.push(FileSpec { content_type: output_type(&name).to_owned(), name, size });
    }
    let answer = match api::post_json::<DoneAnswer>(api, "/api/machine/pool/done", &json!({ "id": id, "ok": true, "outputs": specs })).await {
        Ok(a) => a,
        Err(ApiError::Status { status: 409, .. }) => return Finish::Stopped,
        Err(e) => return Finish::Failed(e.to_string()),
    };
    for (path, spec) in outputs.iter().zip(&specs) {
        let Some(up) = answer.uploads.iter().find(|u| u.name == spec.name) else {
            release(api, id).await;
            return Finish::Released;
        };
        let target = UploadTarget { name: spec.name.clone(), key: String::new(), put_url: up.put_url.clone(), public_url: String::new(), content_type: if up.content_type.is_empty() { spec.content_type.clone() } else { up.content_type.clone() } };
        if upload::put_file(client, &target, path, spec.size, Arc::new(|_| {})).await.is_err() {
            release(api, id).await;
            return Finish::Released;
        }
    }
    match api::post_json::<serde_json::Value>(api, "/api/machine/pool/finish", &json!({ "id": id })).await {
        Ok(_) => Finish::Done,
        Err(e) => Finish::Failed(e.to_string()),
    }
}
