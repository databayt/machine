//! The fixed job templates. A donor never runs what a requester sends: kun hands over a kind and a
//! few parameters, they are validated again here (the same rules as kun's zod schemas), and the
//! command line is built as an argument vector, with no shell, from this file's literals plus the
//! validated values and paths this app made itself.

use std::path::Path;

use serde_json::Value;

/// The kinds a donor may accept (kun's `POOL_KINDS`).
pub const KINDS: [&str; 3] = ["transcribe", "render", "encode"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat {
    Srt,
    Txt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Eevee,
    Cycles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFormat {
    Png,
    Mp4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    H264Web,
    Hevc,
    Proxy,
}

impl Preset {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::H264Web => "h264-web",
            Self::Hevc => "hevc",
            Self::Proxy => "proxy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Params {
    Transcribe { model: Option<String>, language: Option<String>, format: TextFormat },
    Render { engine: Engine, start: u32, end: u32, format: RenderFormat },
    Encode { preset: Preset },
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn frame(v: &Value, key: &str) -> Result<u32, String> {
    let n = v.get(key).and_then(Value::as_u64).ok_or_else(|| format!("{key} must be a whole number"))?;
    if n > 1_000_000 {
        return Err(format!("{key} is too large"));
    }
    u32::try_from(n).map_err(|e| e.to_string())
}

/// Checks a kind and its parameters.
///
/// # Errors
/// A short sentence naming the first rule broken.
pub fn validate(kind: &str, params: &Value) -> Result<Params, String> {
    match kind {
        "transcribe" => {
            let model = match params.get("model") {
                None | Some(Value::Null) => None,
                Some(m) => match m.as_str() {
                    Some(m @ ("base" | "small" | "medium" | "large-v3")) => Some(m.to_owned()),
                    _ => return Err("model must be base, small, medium or large-v3".into()),
                },
            };
            let language = match params.get("language") {
                None | Some(Value::Null) => None,
                Some(l) => match l.as_str() {
                    Some(l) if l.len() == 2 && l.bytes().all(|b| b.is_ascii_lowercase()) => Some(l.to_owned()),
                    _ => return Err("language must be two lowercase letters".into()),
                },
            };
            let format = match str_of(params, "format") {
                Some("srt") => TextFormat::Srt,
                Some("txt") => TextFormat::Txt,
                _ => return Err("format must be srt or txt".into()),
            };
            Ok(Params::Transcribe { model, language, format })
        }
        "render" => {
            let engine = match str_of(params, "engine") {
                Some("eevee") => Engine::Eevee,
                Some("cycles") => Engine::Cycles,
                _ => return Err("engine must be eevee or cycles".into()),
            };
            let (start, end) = (frame(params, "frameStart")?, frame(params, "frameEnd")?);
            if end < start {
                return Err("frameEnd is before frameStart".into());
            }
            let format = match str_of(params, "format") {
                Some("png") => RenderFormat::Png,
                Some("mp4") => RenderFormat::Mp4,
                _ => return Err("format must be png or mp4".into()),
            };
            Ok(Params::Render { engine, start, end, format })
        }
        "encode" => {
            let preset = match str_of(params, "preset") {
                Some("h264-web") => Preset::H264Web,
                Some("hevc") => Preset::Hevc,
                Some("proxy") => Preset::Proxy,
                _ => return Err("preset must be h264-web, hevc or proxy".into()),
            };
            Ok(Params::Encode { preset })
        }
        _ => Err("unknown job kind".into()),
    }
}

/// A command: the program and its argument vector. There is no shell anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn cmd(program: &Path, args: &[&str], tail: &[String]) -> Cmd {
    Cmd { program: s(program), args: args.iter().map(|a| (*a).to_owned()).chain(tail.iter().cloned()).collect() }
}

/// Any audio or video to 16 kHz mono wav, which whisper reads.
#[must_use]
pub fn wav_cmd(ffmpeg: &Path, input: &Path, wav: &Path) -> Cmd {
    cmd(ffmpeg, &["-y", "-nostdin", "-v", "error", "-i"], &[s(input), "-ar".into(), "16000".into(), "-ac".into(), "1".into(), "-c:a".into(), "pcm_s16le".into(), s(wav)])
}

/// `whisper-cli` on a wav; it writes `<out_prefix>.srt` or `.txt`.
#[must_use]
pub fn whisper_cmd(whisper: &Path, model: &Path, wav: &Path, out_prefix: &Path, format: TextFormat, language: Option<&str>) -> Cmd {
    let flag = if format == TextFormat::Srt { "-osrt" } else { "-otxt" };
    cmd(whisper, &["-m"], &[s(model), "-f".into(), s(wav), "-of".into(), s(out_prefix), flag.into(), "-pp".into(), "-l".into(), language.unwrap_or("auto").into()])
}

/// The one ffmpeg line per preset. macOS uses the hardware HEVC encoder, every other system libx265.
#[must_use]
pub fn encode_cmd(ffmpeg: &Path, preset: Preset, mac: bool, input: &Path, output: &Path) -> Cmd {
    let head = ["-y", "-nostdin", "-v", "error", "-progress", "pipe:1", "-nostats", "-i"];
    let codec: &[&str] = match preset {
        Preset::H264Web => &["-c:v", "libx264", "-crf", "23", "-preset", "medium", "-movflags", "+faststart", "-c:a", "aac", "-b:a", "128k"],
        Preset::Hevc if mac => &["-c:v", "hevc_videotoolbox", "-q:v", "55", "-tag:v", "hvc1", "-c:a", "aac", "-b:a", "128k"],
        Preset::Hevc => &["-c:v", "libx265", "-crf", "28", "-preset", "medium", "-tag:v", "hvc1", "-c:a", "aac", "-b:a", "128k"],
        Preset::Proxy => &["-vf", "scale=-2:720", "-c:v", "libx264", "-crf", "28", "-preset", "ultrafast", "-c:a", "aac", "-b:a", "96k"],
    };
    let mut tail = vec![s(input)];
    tail.extend(codec.iter().map(|c| (*c).to_owned()));
    tail.push(s(output));
    cmd(ffmpeg, &head, &tail)
}

/// Blender 5 renamed `BLENDER_EEVEE_NEXT` back to `BLENDER_EEVEE`; 4.2 to 4.5 use `_NEXT`.
#[must_use]
pub fn engine_id(engine: Engine, version: (u32, u32)) -> &'static str {
    match engine {
        Engine::Cycles => "CYCLES",
        Engine::Eevee if version.0 >= 5 || version < (4, 2) => "BLENDER_EEVEE",
        Engine::Eevee => "BLENDER_EEVEE_NEXT",
    }
}

/// `Blender 5.3.0 (hash …)` → (5, 3).
#[must_use]
pub fn parse_blender_version(out: &str) -> Option<(u32, u32)> {
    let line = out.lines().find_map(|l| l.trim().strip_prefix("Blender "))?;
    let mut it = line.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty());
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// Auto-run scripts are off: a `.blend` from a teammate is a program.
#[must_use]
pub fn blender_render_cmd(blender: &Path, engine: &str, file: &Path, out_dir: &Path, start: u32, end: u32) -> Cmd {
    let pattern = out_dir.join("frame_####");
    cmd(blender, &["-b", "--disable-autoexec"], &[s(file), "-E".into(), engine.into(), "-o".into(), s(&pattern), "-F".into(), "PNG".into(), "-s".into(), start.to_string(), "-e".into(), end.to_string(), "-a".into()])
}

/// Prints the scene's frame rate on a `KUNFPS` line (a fixed expression, no input in it).
#[must_use]
pub fn blender_fps_cmd(blender: &Path, file: &Path) -> Cmd {
    cmd(blender, &["-b", "--disable-autoexec"], &[s(file), "--python-expr".into(), "import bpy;print('KUNFPS',bpy.context.scene.render.fps)".into()])
}

#[must_use]
pub fn parse_kun_fps(out: &str) -> Option<u32> {
    out.lines().find_map(|l| l.trim().strip_prefix("KUNFPS")).and_then(|r| r.trim().parse().ok()).filter(|f| (1..=240).contains(f))
}

/// The rendered frames to an mp4.
#[must_use]
pub fn frames_to_mp4_cmd(ffmpeg: &Path, frames_dir: &Path, start: u32, fps: u32, output: &Path) -> Cmd {
    let pattern = frames_dir.join("frame_%04d.png");
    cmd(
        ffmpeg,
        &["-y", "-nostdin", "-v", "error", "-framerate"],
        &[fps.to_string(), "-start_number".into(), start.to_string(), "-i".into(), s(&pattern), "-c:v".into(), "libx264".into(), "-pix_fmt".into(), "yuv420p".into(), "-crf".into(), "18".into(), "-movflags".into(), "+faststart".into(), s(output)],
    )
}

/// Many frames travel as one archive (kun caps the outputs of a job).
#[must_use]
pub fn tar_cmd(archive: &Path, dir: &Path) -> Cmd {
    Cmd { program: "tar".into(), args: vec!["-czf".into(), s(archive), "-C".into(), s(dir), ".".into()] }
}

/// The most PNG frames sent one by one; more go in `frames.tar.gz`.
pub const MAX_LOOSE_FRAMES: usize = 50;

// ---- progress lines ------------------------------------------------------------------------------

/// `out_time_us=1234567` (also `out_time_ms`, which ffmpeg also gives in microseconds) → seconds.
#[must_use]
pub fn ffmpeg_out_time(line: &str) -> Option<f64> {
    let v = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms="))?;
    let us: f64 = v.trim().parse().ok()?;
    (us >= 0.0).then_some(us / 1e6)
}

/// `Fra:12 Mem:…` → 12.
#[must_use]
pub fn blender_frame(line: &str) -> Option<u32> {
    let rest = line.trim_start().strip_prefix("Fra:")?;
    rest.split_whitespace().next()?.parse().ok()
}

/// `whisper_print_progress_callback: progress =  45%` → 0.45.
#[must_use]
pub fn whisper_progress(line: &str) -> Option<f64> {
    let rest = line.split("progress =").nth(1)?;
    let pct: f64 = rest.trim().trim_end_matches('%').trim().parse().ok()?;
    Some((pct / 100.0).clamp(0.0, 1.0))
}

/// ffprobe's `format=duration` answer in seconds.
#[must_use]
pub fn parse_duration(out: &str) -> Option<f64> {
    out.trim().parse::<f64>().ok().filter(|d| *d > 0.0)
}

// ---- names ---------------------------------------------------------------------------------------

/// A local file name made from a requester's: no separators, controls or leading dots.
#[must_use]
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '"' | '*' | '?' | '<' | '>' | '|') { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_owned();
    let cleaned: String = cleaned.chars().take(120).collect();
    if cleaned.is_empty() { "file".to_owned() } else { cleaned }
}

/// The name without its extension.
#[must_use]
pub fn stem(name: &str) -> String {
    name.rsplit_once('.').filter(|(a, _)| !a.is_empty()).map_or(name, |(a, _)| a).to_owned()
}

/// Whisper model file names to look for, best match first.
#[must_use]
pub fn model_file(model: Option<&str>) -> Option<String> {
    model.map(|m| format!("ggml-{m}.bin"))
}
