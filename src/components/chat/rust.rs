//! chat — sends a prompt to the teammate's agent CLI and streams what comes back.
//!
//! `ask` builds a [`driver::Job`] that runs inside the profile's workspace (or the department's
//! pack folder when there is no profile), streams
//! every normalized event down a Tauri `Channel`, and remembers the session so the
//! conversation can be reopened. `stop` cancels a running job; dropping the run kills the
//! child process (`kill_on_drop`), so a stopped job leaves nothing behind.

use std::{
    collections::HashMap,
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{
        LazyLock, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::Serialize;
use tauri::{
    Runtime,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
};
use tokio::sync::oneshot;

use crate::{
    brain::{self, Brain, BrainError},
    driver::{self, DriverError, DriverKind, Event, Job, PermissionMode, Unavailable},
    network::{host::Host, valid_task_id},
    pack::{self, PackError, Workspace},
    profile::{self, OsStore, ProfileError, Resolved},
    sessions,
    settings::{self, SettingsError},
    tasks::{self, SEED_FILE},
};

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("job id is invalid")]
    BadJobId,
    #[error("a job with this id is already running")]
    Duplicate,
    #[error("that repository is not available for this machine or is not cloned here")]
    BadRepo,
    #[error("this department has no route `{0}`")]
    BadRoute(String),
    #[error("{0}")]
    Refine(String),
    #[error("task id is invalid")]
    BadTask,
    #[error("that task was not started here: start it from the task list first")]
    TaskNotStarted,
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Pack(#[from] PackError),
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error(transparent)]
    Brain(#[from] BrainError),
}

impl Serialize for ChatError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[path = "repo_menu.rs"]
pub mod repo_menu;

type Cancel = oneshot::Sender<()>;

/// Running jobs, so `stop` can reach them.
static RUNNING: LazyLock<Mutex<HashMap<String, Cancel>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Removes the job from [`RUNNING`] however the run ends.
struct Registration(String);

impl Drop for Registration {
    fn drop(&mut self) {
        RUNNING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.0);
    }
}

fn register(job_id: &str, cancel: Cancel) -> Result<Registration, ChatError> {
    let mut map = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    if map.contains_key(job_id) {
        return Err(ChatError::Duplicate);
    }
    map.insert(job_id.to_owned(), cancel);
    Ok(Registration(job_id.to_owned()))
}

/// Asks a running job to stop. False when no such job is running.
#[must_use]
pub fn cancel(job_id: &str) -> bool {
    let sender = RUNNING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(job_id);
    sender.is_some_and(|tx| tx.send(()).is_ok())
}

fn valid_job_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The session title: the first prompt, one line, at most 60 characters.
fn title_from(prompt: &str) -> String {
    let line = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        title.push('…');
    }
    if title.is_empty() {
        "…".into()
    } else {
        title
    }
}

/// The flags that give a run its world: every repo of the profile that is on disk, then the
/// generated settings file. Only claude takes them.
fn job_extras(kind: DriverKind, repos: &[String], ws: &Workspace) -> Vec<String> {
    if kind != DriverKind::Claude {
        return Vec::new();
    }
    let mut args = Vec::new();
    for name in repos {
        let dir = ws.repos.join(name);
        if dir.is_dir() {
            args.extend(["--add-dir".to_owned(), dir.display().to_string()]);
        }
    }
    args.extend([
        "--settings".to_owned(),
        ws.settings_file.display().to_string(),
    ]);
    args
}

/// The most `--add-dir` / `--settings` pairs a run may carry (the driver allows 32 args).
const MAX_EXTRA_PAIRS: usize = 16;

/// `--add-dir` pairs for the task's repos that are not already in `extras`. Only claude takes them.
fn task_extras(kind: DriverKind, extras: &[String], repo_dirs: &[PathBuf]) -> Vec<String> {
    if kind != DriverKind::Claude {
        return Vec::new();
    }
    let mut out = Vec::new();
    for dir in repo_dirs {
        let shown = dir.display().to_string();
        if extras.len() / 2 + out.len() / 2 >= MAX_EXTRA_PAIRS {
            break;
        }
        if !extras.contains(&shown) {
            out.extend(["--add-dir".to_owned(), shown]);
        }
    }
    out
}

/// What a task run adds: its repos as `--add-dir`, read from the `TASK.md` that `start` wrote
/// into the working folder. Refuses a task that was not started here.
fn task_setup(
    cwd: &str,
    task_id: &str,
    kind: DriverKind,
    extras: &[String],
    host: &impl Host,
) -> Result<Vec<String>, ChatError> {
    if !valid_task_id(task_id) {
        return Err(ChatError::BadTask);
    }
    let seed = std::fs::read_to_string(Path::new(cwd).join(SEED_FILE))
        .map_err(|_| ChatError::TaskNotStarted)?;
    let names = tasks::readiness::seed_repos(&seed, task_id).ok_or(ChatError::TaskNotStarted)?;
    Ok(task_extras(
        kind,
        extras,
        &tasks::task_repo_dirs(host, &names),
    ))
}

/// The first prompt of a task run points at the seed instead of pasting it.
fn task_prompt(task_id: &str, prompt: &str) -> String {
    format!(
        "You are working on task {task_id}. Read {SEED_FILE} in this folder first: it holds the issue and the repos. Then:\n\n{prompt}"
    )
}

/// The repo a run is limited to. It must be on the repo menu (the department's pack repos and the
/// profile's, active in kun's registry: [`repo_menu::menu_for`]) and be cloned under `root`.
fn validate_repo(
    resolved: &Resolved,
    department: &str,
    repo: Option<&str>,
    root: &Path,
    kun: &Path,
) -> Result<Option<String>, ChatError> {
    let Some(repo) = repo.filter(|r| !r.is_empty()) else {
        return Ok(None);
    };
    let menu = repo_menu::menu_for(kun, &resolved.profile, department)?;
    if menu.iter().any(|e| e.name == repo) && pack::valid_name(repo) && root.join(repo).is_dir() {
        Ok(Some(repo.to_owned()))
    } else {
        Err(ChatError::BadRepo)
    }
}

/// A new session that works in one repo opens with a line saying where; a resumed one already has it.
fn with_preface(prompt: String, repo: Option<&str>, department: &str, is_new: bool) -> String {
    match repo {
        Some(repo) if is_new => format!(
            "Working in {repo} for the {department} department.\n\n{prompt}"
        ),
        _ => prompt,
    }
}

/// The driver for a run: the profile's when it fixes one, else the request, else the saved default.
fn pick_driver(
    resolved: &Resolved,
    requested: Option<DriverKind>,
    saved: DriverKind,
) -> DriverKind {
    if resolved.driver_locked {
        resolved.profile.driver
    } else {
        requested.unwrap_or(saved)
    }
}

fn build_job(
    job_id: &str,
    driver: DriverKind,
    prompt: String,
    cwd: String,
    session: Option<String>,
    extra_args: Vec<String>,
    env: Vec<(String, String)>,
) -> Result<Job, ChatError> {
    if !valid_job_id(job_id) {
        return Err(ChatError::BadJobId);
    }
    Ok(Job {
        job_id: job_id.to_owned(),
        driver,
        prompt,
        cwd,
        session,
        timeout_secs: None,
        extra_args,
        env,
        permission_mode: PermissionMode::Auto,
    })
}

/// Where and how a run starts: its folder, extra flags, environment and timeout, plus the turn's
/// permission mode and the prompt suffix its route adds. Blocking (filesystem, keychain).
struct RunSetup {
    cwd: String,
    extras: Vec<String>,
    env: Vec<(String, String)>,
    timeout: Option<u32>,
    permission: PermissionMode,
    suffix: String,
}

/// What a turn runs as: the pack's route for it, or (old callers) the one the `mode` names.
///
/// `route` wins; with only a `mode`, `plan` / anything else pick the route of that id when the pack has
/// it (`plan` / `do` are what a bare `run` block derives); with neither, the turn is a plain auto run.
/// Only claude takes routes.
fn resolve_turn(
    meta: &pack::PackMeta,
    kind: DriverKind,
    route: Option<&str>,
    mode: Option<&str>,
    free: bool,
) -> Result<(PermissionMode, Vec<String>, String), ChatError> {
    let legacy = PermissionMode::parse(mode);
    if kind != DriverKind::Claude {
        if let Some(id) = route.filter(|id| !meta.routes.iter().any(|r| r.id == *id)) {
            return Err(ChatError::BadRoute(id.to_owned()));
        }
        let permission = route
            .and_then(|id| meta.routes.iter().find(|r| r.id == id))
            .map_or(legacy, pack::Route::permission);
        return Ok((permission, Vec::new(), String::new()));
    }
    let found = match (route, mode) {
        (Some(id), _) => pack::find_route(meta, Some(id))
            .map_err(|_| ChatError::BadRoute(id.to_owned()))?,
        (None, Some(_)) => {
            let id = if legacy == PermissionMode::Plan { "plan" } else { "do" };
            meta.routes.iter().find(|r| r.id == id)
        }
        (None, None) => None,
    };
    Ok(found.map_or((legacy, Vec::new(), String::new()), |r| {
        (r.permission(), pack::route_flags_for(r, free), r.suffix.clone())
    }))
}

/// The prompt with its route's suffix: `\n\n<suffix>`, every turn.
fn with_suffix(prompt: String, suffix: &str) -> String {
    if suffix.is_empty() {
        prompt
    } else {
        format!("{prompt}\n\n{suffix}")
    }
}

#[allow(clippy::too_many_arguments)] // one named input per thing a run depends on
fn prepare_run(
    resolved: &Resolved,
    department: &str,
    kind: DriverKind,
    task_id: Option<&str>,
    repo: Option<&str>,
    root: &Path,
    route: Option<&str>,
    mode: Option<&str>,
    brain: Brain,
) -> Result<RunSetup, ChatError> {
    let (cwd, mut extras, mut env) = if resolved.explicit {
        let ws = pack::workspace(&resolved.profile)?;
        let names = repo.map_or_else(|| resolved.profile.repos.clone(), |r| vec![r.to_owned()]);
        (
            ws.dir.display().to_string(),
            job_extras(kind, &names, &ws),
            profile::secret_env(&OsStore, resolved)?,
        )
    } else {
        let extras = match repo {
            Some(r) if kind == DriverKind::Claude => {
                vec!["--add-dir".to_owned(), root.join(r).display().to_string()]
            }
            _ => Vec::new(),
        };
        (
            pack::workdir(department)?.display().to_string(),
            extras,
            Vec::new(),
        )
    };
    // The department's own world: extra folders (a longer timeout rides along).
    let meta = pack::meta_in(&pack::kun_dir()?, department)?;
    let dirs = task_extras(kind, &extras, &pack::pack_dirs(&meta));
    extras.extend(dirs);
    let (permission, flags, suffix) = resolve_turn(&meta, kind, route, mode, brain.is_free())?;
    extras.extend(flags);
    if let Some(id) = task_id {
        let seeded = task_setup(&cwd, id, kind, &extras, &crate::network::host::RealHost)?;
        extras.extend(seeded);
    }
    // A free route: start (or adopt) its proxy and point claude at it. Only claude takes it, and a
    // route that is not ready stops the run here, never falling back to Claude.
    if kind == DriverKind::Claude && brain.is_free() {
        let add = brain::prepare(brain, resolved, &OsStore)?;
        env = brain::merge_env(env, add);
    }
    Ok(RunSetup {
        cwd,
        extras,
        env,
        timeout: meta.timeout_secs,
        permission,
        suffix,
    })
}

/// `ask` — runs one prompt, streaming events to `channel`. The frontend supplies `job_id`
/// so it can call `stop` before this command returns. With `task_id` the run is a task's: its
/// repos join as `--add-dir` and the prompt points at `TASK.md`. With `repo` the run is limited to
/// that repository (`--add-dir` only it) and a new session opens with a line naming it and the
/// department. `route` is one of the department's route ids (`pack.json` `routes`): it sets the
/// permission mode, model, effort and agent, and its suffix is appended to the prompt; an unknown id
/// rejects the call. `mode` (`"plan"` | `"auto"`, anything else is auto) is the older way, used when
/// `route` is absent: it picks the route called `plan` or `do`. Run-time failures arrive as an
/// `error` event; only bad arguments reject the call.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // the IPC surface: one named argument per field the UI sends
async fn ask(
    prompt: String,
    session: Option<String>,
    driver: Option<DriverKind>,
    job_id: String,
    task_id: Option<String>,
    repo: Option<String>,
    mode: Option<String>,
    route: Option<String>,
    channel: Channel<Event>,
) -> Result<(), ChatError> {
    let settings = settings::load()?;
    let resolved = profile::current()?;
    let kind = pick_driver(&resolved, driver, settings.driver);
    let department = profile::effective_department(&resolved.profile, &settings.department);
    let root = crate::repos::repos_root().map_err(|e| ChatError::Pack(PackError::Profile(e.to_string())))?;
    let repo = validate_repo(&resolved, &department, repo.as_deref(), &root, &pack::kun_dir()?)?;
    let inputs = SetupInputs { resolved, department, kind, task_id, repo, root, route, mode };
    let configured = settings.brain;
    // A session that already fell back stays on its free route (when it is still ready).
    let sticky = if kind == DriverKind::Claude && !configured.is_free() {
        match session.as_deref().and_then(brain::fallback_get) {
            Some(b) if ready_routes().await.contains(&b) => Some(b),
            _ => None,
        }
    } else {
        None
    };
    let first = sticky.unwrap_or(configured);
    let setup = inputs.setup(first).await?;
    let title = title_from(&prompt);
    let ctx = RunCtx { title, department: inputs.department.clone(), repo: inputs.repo.clone(), kind };
    let arabic = is_arabic(&prompt);
    let (tx, mut rx) = oneshot::channel();
    let _registration = register(&job_id, tx)?;
    let mut seen = session.clone();
    let mut notice = sticky.map(|b| still_notice(b, arabic));
    let make_job = |setup: RunSetup| -> Result<Job, ChatError> {
        let text = assemble_prompt(&prompt, &inputs, session.is_some(), &setup.suffix);
        let mut job = build_job(&job_id, kind, text, setup.cwd, session.clone(), setup.extras, setup.env)?;
        job.timeout_secs = setup.timeout;
        job.permission_mode = setup.permission;
        Ok(job)
    };
    let job = make_job(setup)?;
    if let Some(n) = notice.take() {
        let _ = channel.send(Event::Text { text: n });
    }
    // Attempt 0 on the chosen route (held back while it might turn out to be "Claude unavailable").
    let gated = kind == DriverKind::Claude && !first.is_free();
    let mut end = run_job(&job, &mut rx, gated, &ctx, &channel, &mut seen).await;
    // Attempt 1, once: Claude was unavailable. Never a loop: the fallback run is not gated or retried.
    end = match end {
        RunEnd::Unavailable(why) if should_fall_back(kind, configured, 0, Some(&why)) => {
            fall_back(&inputs, &make_job, &why, arabic, &mut rx, &ctx, &channel, &mut seen).await
        }
        other => other,
    };
    let last = match end {
        RunEnd::Finished(Ok(())) => None,
        RunEnd::Finished(Err(e)) => Some(Event::Error { message: e.to_string() }),
        RunEnd::Failed(message) => Some(Event::Error { message }),
        RunEnd::Stopped => Some(Event::Done { session: seen, ok: false, result: Some("stopped".into()) }),
        RunEnd::Unavailable(why) => Some(Event::Error { message: no_route_message(&why, None, arabic) }),
    };
    if let Some(event) = last {
        let _ = channel.send(event);
    }
    Ok(())
}

/// Everything needed to prepare a run, kept so a fallback can prepare the same run on another route.
struct SetupInputs {
    resolved: Resolved,
    department: String,
    kind: DriverKind,
    task_id: Option<String>,
    repo: Option<String>,
    root: PathBuf,
    route: Option<String>,
    mode: Option<String>,
}

impl SetupInputs {
    async fn setup(&self, brain: Brain) -> Result<RunSetup, ChatError> {
        let (resolved, department, kind, task_id, repo, root, route, mode) = (
            self.resolved.clone(),
            self.department.clone(),
            self.kind,
            self.task_id.clone(),
            self.repo.clone(),
            self.root.clone(),
            self.route.clone(),
            self.mode.clone(),
        );
        tokio::task::spawn_blocking(move || {
            prepare_run(&resolved, &department, kind, task_id.as_deref(), repo.as_deref(), &root, route.as_deref(), mode.as_deref(), brain)
        })
        .await
        .map_err(|e| ChatError::Pack(PackError::Profile(e.to_string())))?
    }
}

/// The prompt as sent: a task's seed, the repo/department preface (new sessions), the route's suffix.
fn assemble_prompt(prompt: &str, inputs: &SetupInputs, resumed: bool, suffix: &str) -> String {
    // The seed is only the opening move: a resumed session already has it.
    let text = match (&inputs.task_id, resumed) {
        (Some(id), false) => task_prompt(id, prompt),
        _ => prompt.to_owned(),
    };
    let text = with_preface(text, inputs.repo.as_deref(), &inputs.department, !resumed);
    with_suffix(text, suffix)
}

/// What the session record and the sink need to know about a run.
struct RunCtx {
    title: String,
    department: String,
    repo: Option<String>,
    kind: DriverKind,
}

/// How one driver run ended.
enum RunEnd {
    /// The driver returned (its own `Done` / `Error` events already went out).
    Finished(Result<(), DriverError>),
    /// The person stopped it.
    Stopped,
    /// Claude was unavailable; nothing of the run reached the UI.
    Unavailable(Unavailable),
    /// The fallback could not run: one line for the UI.
    Failed(String),
}

/// Holds a Claude run's opening events until it is clear the run is not just "Claude unavailable":
/// the CLI prints its sign-in / limit line as an assistant message and a failed result, which must
/// not reach the chat as an answer when a fallback is about to take over. Pure.
#[derive(Debug, Default)]
struct Gate {
    held: Vec<Event>,
    committed: bool,
    hit: Option<Unavailable>,
}

impl Gate {
    /// Takes one event; returns what may be shown now.
    fn push(&mut self, event: Event) -> Vec<Event> {
        if self.committed {
            return vec![event];
        }
        match &event {
            Event::Started { .. } => self.held.push(event),
            Event::Text { text } if driver::classify_unavailable(text).is_some() => self.held.push(event),
            Event::Done { ok: false, result: Some(r), .. } if driver::classify_unavailable(r).is_some() => {
                self.hit = driver::classify_unavailable(r);
                self.held.push(event);
            }
            Event::Error { message } if driver::classify_unavailable(message).is_some() => {
                self.hit = driver::classify_unavailable(message);
                self.held.push(event);
            }
            _ => {
                self.committed = true;
                let mut out = std::mem::take(&mut self.held);
                out.push(event);
                return out;
            }
        }
        Vec::new()
    }

    /// The run is over. `Err(why)` when it failed as unavailable (the held events are dropped);
    /// `Ok(events)` is whatever was still held back.
    fn finish(&mut self) -> Result<Vec<Event>, Unavailable> {
        match self.hit.take() {
            Some(why) if !self.committed => Err(why),
            _ => Ok(std::mem::take(&mut self.held)),
        }
    }
}

/// Records the session and sends the event to the UI.
fn deliver(event: Event, ctx: &RunCtx, seen: &mut Option<String>, channel: &Channel<Event>) {
    let id = match &event {
        Event::Started { session } => Some(session.clone()),
        Event::Done { session: Some(s), .. } => Some(s.clone()),
        _ => None,
    };
    if let Some(id) = id {
        if let Err(e) = sessions::touch(&id, &ctx.title, &ctx.department, ctx.repo.as_deref(), ctx.kind) {
            tracing::warn!("could not record session: {e}");
        }
        *seen = Some(id);
    }
    if channel.send(event).is_err() {
        tracing::debug!("chat channel closed");
    }
}

/// Runs `job` once, streaming to the UI. With `gated` the opening events are held (see [`Gate`]) and a
/// run that fails as "Claude unavailable" ends as [`RunEnd::Unavailable`] with nothing shown.
async fn run_job(
    job: &Job,
    rx: &mut oneshot::Receiver<()>,
    gated: bool,
    ctx: &RunCtx,
    channel: &Channel<Event>,
    seen: &mut Option<String>,
) -> RunEnd {
    let mut gate = Gate { committed: !gated, ..Gate::default() };
    let outcome = {
        let emit = |event: Event| {
            for e in gate.push(event) {
                deliver(e, ctx, seen, channel);
            }
        };
        tokio::select! {
            r = driver::run(job, emit) => Some(r),
            _ = &mut *rx => None,
        }
    };
    match gate.finish() {
        Err(why) if outcome.is_some() => return RunEnd::Unavailable(why),
        Err(_) => {}
        Ok(held) => {
            for e in held {
                deliver(e, ctx, seen, channel);
            }
        }
    }
    outcome.map_or(RunEnd::Stopped, RunEnd::Finished)
}

/// Routes that are installed with a key, in fallback order. Blocking work off the async thread.
async fn ready_routes() -> Vec<Brain> {
    tokio::task::spawn_blocking(|| brain::view(&OsStore).map(|v| brain::ready_fallbacks(&v.routes)).unwrap_or_default())
        .await
        .unwrap_or_default()
}

/// Whether a failed run should be re-run on a free route: only a Claude-driver run on the Claude
/// route, on its first attempt, that failed as "Claude unavailable".
fn should_fall_back(kind: DriverKind, configured: Brain, attempt: u8, why: Option<&Unavailable>) -> bool {
    kind == DriverKind::Claude && !configured.is_free() && attempt == 0 && why.is_some()
}

/// Re-runs the job once on the first ready free route, telling the UI first. Never silent, never an
/// API key, never a second fallback.
#[allow(clippy::too_many_arguments)] // the pieces of one run
async fn fall_back(
    inputs: &SetupInputs,
    make_job: &impl Fn(RunSetup) -> Result<Job, ChatError>,
    why: &Unavailable,
    arabic: bool,
    rx: &mut oneshot::Receiver<()>,
    ctx: &RunCtx,
    channel: &Channel<Event>,
    seen: &mut Option<String>,
) -> RunEnd {
    let ready = ready_routes().await;
    if ready.is_empty() {
        return RunEnd::Failed(no_route_message(why, None, arabic));
    }
    let mut last_error = None;
    for route in ready {
        match inputs.setup(route).await {
            Ok(setup) => {
                let job = match make_job(setup) {
                    Ok(j) => j,
                    Err(e) => return RunEnd::Failed(e.to_string()),
                };
                let _ = channel.send(Event::Text { text: fallback_notice(why, route, arabic) });
                let end = run_job(&job, rx, false, ctx, channel, seen).await;
                if let Some(id) = seen.as_deref() {
                    brain::fallback_set(id, route);
                }
                return end;
            }
            Err(e) => last_error = Some((route, e.to_string())),
        }
    }
    RunEnd::Failed(no_route_message(why, last_error, arabic))
}

fn is_arabic(text: &str) -> bool {
    text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

/// Why Claude is unavailable, in a few words.
fn reason_text(why: &Unavailable, arabic: bool) -> String {
    match (why, arabic) {
        (Unavailable::NotSignedIn, false) => "not signed in".into(),
        (Unavailable::NoSubscription, false) => "no subscription".into(),
        (Unavailable::UsageLimit { resets_at: Some(t) }, false) => format!("usage limit, resets {t}"),
        (Unavailable::UsageLimit { resets_at: None }, false) => "usage limit".into(),
        (Unavailable::NotSignedIn, true) => "لم يتم تسجيل الدخول".into(),
        (Unavailable::NoSubscription, true) => "لا يوجد اشتراك".into(),
        (Unavailable::UsageLimit { resets_at: Some(t) }, true) => format!("بلغت حد الاستخدام، يتجدد {t}"),
        (Unavailable::UsageLimit { resets_at: None }, true) => "بلغت حد الاستخدام".into(),
    }
}

/// The line shown (as agent text) before a fallback run. Names the route and the privacy caveat.
fn fallback_notice(why: &Unavailable, route: Brain, arabic: bool) -> String {
    let (reason, label) = (reason_text(why, arabic), route.label());
    if arabic {
        format!("ملاحظة: Claude غير متاح ({reason}). هذا التشغيل يستخدم {label} مجانًا، والمزوّد المجاني يستطيع قراءة ما يُرسل إليه.")
    } else {
        format!("Note: Claude isn't available ({reason}). This run uses {label}, free. A free provider can read what it sends.")
    }
}

/// The line shown when a session stays on its earlier fallback.
fn still_notice(route: Brain, arabic: bool) -> String {
    let label = route.label();
    if arabic {
        format!("ملاحظة: هذه الجلسة تبقى على {label} مجانًا لأن Claude لم يكن متاحًا سابقًا، والمزوّد المجاني يستطيع قراءة ما يُرسل إليه.")
    } else {
        format!("Note: this session stays on {label}, free, because Claude wasn't available earlier. A free provider can read what it sends.")
    }
}

/// The end-of-run error when no free route could take over.
fn no_route_message(why: &Unavailable, tried: Option<(Brain, String)>, arabic: bool) -> String {
    let reason = reason_text(why, arabic);
    match (tried, arabic) {
        (None, false) => format!("Claude isn't available ({reason}) and no free route is ready. Open Settings → Model to set one up."),
        (Some((b, e)), false) => format!("Claude isn't available ({reason}), and {} could not start: {e} Check Settings → Model.", b.label()),
        (None, true) => format!("Claude غير متاح ({reason}) ولا يوجد مسار مجاني جاهز. افتح الإعدادات ← النموذج لإعداده."),
        (Some((b, e)), true) => format!("Claude غير متاح ({reason})، وتعذّر تشغيل {}: {e} راجع الإعدادات ← النموذج.", b.label()),
    }
}

/// How long `refine` may take before it gives up: it reads the project's folder first.
const REFINE_SECS: u32 = 240;

/// The token in a pack's `gather` entry that becomes ` --repo <name>` when a repo is chosen, else nothing.
pub const GATHER_REPO_FLAG: &str = "{repo_flag}";

/// The instruction `refine` puts before the user's text. The brief is always English and follows
/// the pack's predefined `formula.en` template when it has one (a plain section list otherwise); it
/// must stand alone, so the run first gathers context read-only: the department's CLAUDE.md and
/// everything in the pack's `gather` list (one thing to read per entry; no department is special).
/// `routes` adds a closing `route:` line the app reads silently.
fn refine_prompt(
    prompt: &str,
    routes: &[pack::Route],
    gather: &[String],
    repo: Option<&str>,
    template: Option<&str>,
) -> String {
    let mut out = String::from(
        "Rewrite the request below as one professional prompt, in English whatever language the \
         request is in. First gather the context it needs, read-only (no writes, no renders): the \
         department's CLAUDE.md",
    );
    let repo_flag = repo
        .filter(|r| !r.is_empty())
        .map_or_else(String::new, |r| format!(" --repo {r}"));
    for item in gather.iter().map(|g| g.trim()).filter(|g| !g.is_empty()) {
        let _ = write!(out, "; {}", item.replace(GATHER_REPO_FLAG, &repo_flag));
    }
    out.push_str(
        ". Put every reference and fact the worker needs into the prompt (absolute paths, CDN URLs, \
         project ID, stage and gate, price, currency, dialect, length), so it stands alone. Do not \
         start the work.",
    );
    match template.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            let _ = write!(
                out,
                " Fill this template exactly — same headings, same order; write \"none\" where nothing \
                 applies:\n\n{t}\n\n"
            );
        }
        None => out.push_str(
            " Use these headings in order: Task · Context · References · Constraints · Deliverable.",
        ),
    }
    out.push_str(" Output only the prompt");
    if routes.is_empty() {
        out.push('.');
    } else {
        let list = routes.iter().map(|r| r.id.as_str()).collect::<Vec<_>>().join(", ");
        let _ = write!(out, ", then on the very last line `route: <id>`, one of: {list}.");
    }
    format!("{out}\n\n---\n\n{}", prompt.trim())
}

/// A job id for a refine call: unique within the process, valid for [`valid_job_id`].
fn refine_job_id() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("refine-{nanos}-{}", N.fetch_add(1, Ordering::Relaxed))
}

/// The brief: the run's final result when it has one, else the text it streamed.
fn refined_text(streamed: &str, result: Option<&str>) -> Result<String, ChatError> {
    let text = result
        .filter(|r| !r.trim().is_empty())
        .unwrap_or(streamed)
        .trim();
    if text.is_empty() {
        Err(ChatError::Driver(DriverError::Invalid("empty answer")))
    } else {
        Ok(text.to_owned())
    }
}

/// `refine` — rewrites rough text as a professional brief for the department (and, when the pack has
/// routes, ends with a `route: <id>` line). One fresh plan-mode Claude turn on sonnet at medium effort,
/// 240 seconds at most, no session kept.
#[tauri::command]
async fn refine(prompt: String, repo: Option<String>) -> Result<String, ChatError> {
    let settings = settings::load()?;
    let resolved = profile::current()?;
    let department = profile::effective_department(&resolved.profile, &settings.department);
    let root = crate::repos::repos_root().map_err(|e| ChatError::Pack(PackError::Profile(e.to_string())))?;
    let repo = validate_repo(&resolved, &department, repo.as_deref(), &root, &pack::kun_dir()?)?;
    let for_dept = department.clone();
    let repo_name = repo.clone();
    let brain = settings.brain;
    let (cwd, extras, env, routes, gather, template) = tokio::task::spawn_blocking(move || {
        let setup = prepare_run(
            &resolved,
            &for_dept,
            DriverKind::Claude,
            None,
            repo.as_deref(),
            &root,
            None,
            None,
            brain,
        )?;
        let meta = pack::meta_in(&pack::kun_dir()?, &for_dept)?;
        // No route applies (no agent, no pack model): refine is a cheap rewrite. On a free route
        // the proxy picks the model, so only the effort stays.
        let mut extras = setup.extras;
        if !brain.is_free() {
            extras.extend(["--model", "sonnet"].map(str::to_owned));
        }
        extras.extend(["--effort", "medium"].map(str::to_owned));
        let template = pack::info_in(&pack::kun_dir()?, &for_dept)?.formula.map(|f| f.en);
        Ok::<_, ChatError>((setup.cwd, extras, setup.env, meta.routes, meta.gather, template))
    })
    .await
    .map_err(|e| ChatError::Pack(PackError::Profile(e.to_string())))??;
    let mut job = build_job(
        &refine_job_id(),
        DriverKind::Claude,
        refine_prompt(&prompt, &routes, &gather, repo_name.as_deref(), template.as_deref()),
        cwd,
        None,
        extras,
        // Refine has never carried the profile's secrets; on a free route it carries only the
        // route's own variables.
        env.into_iter().filter(|(k, _)| k.starts_with("ANTHROPIC_")).collect(),
    )?;
    job.timeout_secs = Some(REFINE_SECS);
    job.permission_mode = PermissionMode::Plan;
    let (tx, rx) = oneshot::channel();
    let _registration = register(&job.job_id, tx)?;

    let (mut streamed, mut result, mut failure) = (String::new(), None, None);
    let outcome = {
        let emit = |event: Event| match event {
            Event::Text { text } => streamed.push_str(&text),
            Event::Done { result: r, .. } => result = r,
            Event::Error { message } => failure = Some(message),
            _ => {}
        };
        tokio::select! {
            r = driver::run(&job, emit) => Some(r),
            _ = rx => None,
        }
    };
    match outcome {
        Some(Ok(())) => {}
        Some(Err(e)) => return Err(e.into()),
        None => return Err(ChatError::Driver(DriverError::Invalid("stopped"))),
    }
    if let Some(message) = failure {
        return Err(ChatError::Refine(message));
    }
    refined_text(&streamed, result.as_deref())
}

/// `repo_menu` — the repositories the menu offers for a department on this machine, hogwarts first.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn repo_menu(department: String) -> Result<Vec<repo_menu::RepoEntry>, ChatError> {
    let resolved = profile::current()?;
    Ok(repo_menu::menu_for(&pack::kun_dir()?, &resolved.profile, &department)?)
}

/// `stop` — cancels a running job. True when it was running.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn stop(job_id: String) -> bool {
    cancel(&job_id)
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("chat")
        .invoke_handler(tauri::generate_handler![ask, refine, stop, repo_menu])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
