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
    driver::{self, DriverError, DriverKind, Event, Job, PermissionMode},
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
}

impl Serialize for ChatError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

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

/// The repos the chat's repository menu offers on every machine (databayt on GitHub); hogwarts first,
/// the default. Mirrors `CHAT_REPOS` in `chat/context.tsx`.
pub const CHAT_REPOS: &[&str] = &[
    "hogwarts",
    "mkan",
    "kun",
    "machine",
    "marketing",
    "android-app",
    "ios-app",
];

/// The repo a run is limited to. It must be one the machine works in (one of `CHAT_REPOS`, the
/// profile's repos, or when the profile lists none, the department's) and be cloned under `root`.
fn validate_repo(
    resolved: &Resolved,
    department: &str,
    repo: Option<&str>,
    root: &Path,
) -> Result<Option<String>, ChatError> {
    let Some(repo) = repo.filter(|r| !r.is_empty()) else {
        return Ok(None);
    };
    let profile = &resolved.profile;
    let known = CHAT_REPOS.contains(&repo) || if profile.repos.is_empty() {
        profile
            .departments
            .iter()
            .filter(|d| d.name == department)
            .flat_map(|d| d.repos.iter())
            .any(|r| r.name == repo)
    } else {
        profile.repos.iter().any(|r| r == repo)
    };
    if known && pack::valid_name(repo) && root.join(repo).is_dir() {
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
        (r.permission(), pack::route_flags(r), r.suffix.clone())
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
) -> Result<RunSetup, ChatError> {
    let (cwd, mut extras, env) = if resolved.explicit {
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
    let (permission, flags, suffix) = resolve_turn(&meta, kind, route, mode)?;
    extras.extend(flags);
    if let Some(id) = task_id {
        let seeded = task_setup(&cwd, id, kind, &extras, &crate::network::host::RealHost)?;
        extras.extend(seeded);
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
    let repo = validate_repo(&resolved, &department, repo.as_deref(), &root)?;
    let for_run = task_id.clone();
    let (for_dept, for_repo) = (department.clone(), repo.clone());
    let setup = tokio::task::spawn_blocking(move || {
        prepare_run(
            &resolved,
            &for_dept,
            kind,
            for_run.as_deref(),
            for_repo.as_deref(),
            &root,
            route.as_deref(),
            mode.as_deref(),
        )
    })
    .await
    .map_err(|e| ChatError::Pack(PackError::Profile(e.to_string())))??;
    let RunSetup { cwd, extras, env, timeout, permission, suffix } = setup;
    let title = title_from(&prompt);
    // The seed is only the opening move: a resumed session already has it.
    let prompt = match (&task_id, &session) {
        (Some(id), None) => task_prompt(id, &prompt),
        _ => prompt,
    };
    let prompt = with_preface(prompt, repo.as_deref(), &department, session.is_none());
    let prompt = with_suffix(prompt, &suffix);
    let mut job = build_job(&job_id, kind, prompt, cwd, session, extras, env)?;
    job.timeout_secs = timeout;
    job.permission_mode = permission;
    let (tx, rx) = oneshot::channel();
    let _registration = register(&job.job_id, tx)?;

    let mut seen = job.session.clone();
    let outcome = {
        let emit = |event: Event| {
            let id = match &event {
                Event::Started { session } => Some(session.clone()),
                Event::Done {
                    session: Some(s), ..
                } => Some(s.clone()),
                _ => None,
            };
            if let Some(id) = id {
                if let Err(e) = sessions::touch(&id, &title, &department, repo.as_deref(), kind) {
                    tracing::warn!("could not record session: {e}");
                }
                seen = Some(id);
            }
            if channel.send(event).is_err() {
                tracing::debug!("chat channel closed");
            }
        };
        tokio::select! {
            r = driver::run(&job, emit) => Some(r),
            _ = rx => None,
        }
    };
    let last = match outcome {
        Some(Ok(())) => None,
        Some(Err(e)) => Some(Event::Error {
            message: e.to_string(),
        }),
        None => Some(Event::Done {
            session: seen,
            ok: false,
            result: Some("stopped".into()),
        }),
    };
    if let Some(event) = last {
        let _ = channel.send(event);
    }
    Ok(())
}

/// How long `refine` may take before it gives up: it reads the project's folder first.
const REFINE_SECS: u32 = 240;

/// The instruction `refine` puts before the user's text. The brief must stand alone, so the run first
/// gathers context (read-only); the video department also reads its produce projects. `routes` adds
/// the closing line that picks one.
fn refine_prompt(prompt: &str, routes: &[pack::Route], department: &str, repo: Option<&str>) -> String {
    let mut out = String::from(
        "Rewrite the request below as one complete, professional brief for this department. First gather \
         the context it needs, read-only (no writes, no renders): the department's CLAUDE.md formula",
    );
    if department == "video" {
        let repo_flag = repo.map_or_else(String::new, |r| format!(" --repo {r}"));
        let _ = write!(
            out,
            "; for video, `node ~/.claude/skills/produce/scripts/produce.mjs list --json{repo_flag}` and \
             `status <ID>` of the matching project, its folder (reference video, transcript, script, \
             asset manifest), kun/content/media/brand-kit.json and markets.json (price, currency, \
             dialect), existing captures"
        );
    }
    out.push_str(
        ". Then write the brief: what · for whom · version · goal · destination · then a **References & \
         context** section listing every absolute path, CDN URL, project ID, current stage and gate, \
         and key facts (price, currency, dialect, length) the worker needs, so the prompt stands alone \
         · then what to return. Keep the request's language. Do not start the work. Output only the brief",
    );
    if routes.is_empty() {
        out.push('.');
    } else {
        let list = routes
            .iter()
            .map(|r| format!("{} — {}", r.id, r.label_en))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(out, ", then on the very last line `route: <id>` choosing one of: {list}.");
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
    let repo = validate_repo(&resolved, &department, repo.as_deref(), &root)?;
    let for_dept = department.clone();
    let repo_name = repo.clone();
    let (cwd, extras, routes) = tokio::task::spawn_blocking(move || {
        let setup = prepare_run(
            &resolved,
            &for_dept,
            DriverKind::Claude,
            None,
            repo.as_deref(),
            &root,
            None,
            None,
        )?;
        let meta = pack::meta_in(&pack::kun_dir()?, &for_dept)?;
        // No route applies (no agent, no pack model): refine is a cheap rewrite.
        let mut extras = setup.extras;
        extras.extend(["--model", "sonnet", "--effort", "medium"].map(str::to_owned));
        Ok::<_, ChatError>((setup.cwd, extras, meta.routes))
    })
    .await
    .map_err(|e| ChatError::Pack(PackError::Profile(e.to_string())))??;
    let mut job = build_job(
        &refine_job_id(),
        DriverKind::Claude,
        refine_prompt(&prompt, &routes, &department, repo_name.as_deref()),
        cwd,
        None,
        extras,
        Vec::new(),
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

/// `stop` — cancels a running job. True when it was running.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn stop(job_id: String) -> bool {
    cancel(&job_id)
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("chat")
        .invoke_handler(tauri::generate_handler![ask, refine, stop])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
