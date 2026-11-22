//! tasks — the task gate. A task is a GitHub issue; kun's API lists and claims them, this side
//! decides whether *this machine* may start one (`readiness.rs`) and gives the chat everything
//! the work needs: `TASK.md` in the working folder, and the required repos opened with the run.
//!
//! `start` re-checks the gate, claims on the server (atomic there), writes the seed and remembers
//! the held task for the heartbeat. Nothing is skipped silently: only the CEO closes an issue.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;
use tauri::{
    Runtime,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
};

use crate::{
    network::{
        api::{self, Api, ApiError, HttpApi},
        host::{self, Host, RealHost},
        model::Receipt,
        valid_task_id,
    },
    pack, profile, settings,
};

#[path = "model.rs"]
pub mod model;
#[path = "readiness.rs"]
pub mod readiness;
#[path = "upload.rs"]
pub mod upload;

use model::{Context, ReadyState, Task, TaskDetail, TasksResponse, TasksView};
use readiness::Facts;

/// The seed's file name, in the working folder.
pub const SEED_FILE: &str = "TASK.md";

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("task not found")]
    NotFound,
    #[error("not ready: {}", .0.join(", "))]
    NotReady(Vec<String>),
    #[error("kun refused: {0}")]
    Refused(String),
    #[error("{0}")]
    Local(String),
}

impl Serialize for TaskError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// What the machine looks like right now. Blocking (git, filesystem): call off the async thread.
#[must_use]
pub fn gather_facts(host: &impl Host) -> Facts {
    Facts {
        me: host.login(),
        repos: host.repos(),
        items: host.item_states().unwrap_or_default(),
        allowed_items: host.allowed_items(),
        allowed_repos: host.allowed_repos(),
    }
}

/// The folder a run starts in: the profile's workspace, else the department's pack folder.
fn work_dir() -> Result<PathBuf, TaskError> {
    let local = |e: String| TaskError::Local(e);
    let s = settings::load().map_err(|e| local(e.to_string()))?;
    let resolved = profile::current().map_err(|e| local(e.to_string()))?;
    if resolved.explicit {
        Ok(pack::workspace(&resolved.profile)
            .map_err(|e| local(e.to_string()))?
            .dir)
    } else {
        pack::workdir(&profile::effective_department(&resolved.profile, &s.department)).map_err(|e| local(e.to_string()))
    }
}

fn departments_to_search() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(s) = settings::load_effective() {
        out.push(s.department);
    }
    if let Ok(r) = profile::current() {
        for d in r.profile.departments {
            if !out.contains(&d.name) {
                out.push(d.name);
            }
        }
    }
    out
}

/// The tasks of a department with a verdict for each.
///
/// # Errors
/// When the API call fails.
pub async fn view_with(
    api: &impl Api,
    facts: &Facts,
    department: &str,
) -> Result<TasksView, TaskError> {
    let tasks: TasksResponse =
        api::get_json(api, &format!("/api/machine/tasks?department={department}")).await?;
    Ok(readiness::view(tasks, facts))
}

/// A refusal arrives as `{ error, detail? }`: always on a 409, and on any other 4xx that carries one.
/// The reason is the server's, word then detail, so the person reads what kun said.
fn refusal(err: ApiError) -> TaskError {
    if let ApiError::Status { status, body } = &err
        && (400..500).contains(status)
    {
        let parsed: Option<serde_json::Value> = serde_json::from_str(body).ok();
        let word = parsed
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(|e| e.as_str());
        let detail = parsed
            .as_ref()
            .and_then(|v| v.get("detail"))
            .and_then(|e| e.as_str());
        match (word, detail) {
            (Some(w), Some(d)) => return TaskError::Refused(format!("{w}: {d}")),
            (Some(w), None) => return TaskError::Refused(w.to_owned()),
            _ if *status == 409 => return TaskError::Refused(body.clone()),
            _ => {}
        }
    }
    TaskError::Api(err)
}

/// Finds a task by id across departments.
async fn find_task(
    api: &impl Api,
    facts: &Facts,
    departments: &[String],
    id: &str,
) -> Result<(Task, TasksView), TaskError> {
    for department in departments {
        let view = view_with(api, facts, department).await?;
        let hit = view
            .tasks
            .mine
            .iter()
            .chain(&view.tasks.board)
            .find(|t| t.id == id)
            .cloned();
        if let Some(task) = hit {
            return Ok((task, view));
        }
    }
    Err(TaskError::NotFound)
}

/// The machine-wide state file a run's seed quotes from: `KUN_AWARE`, else `~/.claude/state/aware.md`.
fn aware_path() -> Option<PathBuf> {
    match std::env::var_os("KUN_AWARE") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => dirs::home_dir().map(|h| h.join(".claude/state/aware.md")),
    }
}

/// [`start_with_aware`] without the state excerpt.
///
/// # Errors
/// As [`start_with_aware`].
pub async fn start_with(
    api: &impl Api,
    facts: &Facts,
    departments: &[String],
    dir: &Path,
    id: &str,
) -> Result<Task, TaskError> {
    start_with_aware(api, facts, departments, dir, id, None).await
}

/// The context pack, best effort: the last comments from kun and the `aware` lines about the repos.
/// A failure only means a thinner `TASK.md`.
async fn gather_context(api: &impl Api, task: &Task, aware: Option<&Path>) -> Context {
    let comments = api::get_json::<TaskDetail>(api, &format!("/api/machine/task?id={}", task.id.replace('#', "%23")))
        .await
        .map(|d| d.comments)
        .unwrap_or_default();
    let aware = aware
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| readiness::aware_excerpt(&t, &task.kun.repos));
    Context { comments, aware }
}

/// Re-checks the gate, claims on the server, writes the seed (with the context pack) into `dir`.
/// Returns the task.
///
/// # Errors
/// [`TaskError::NotReady`] when the gate says no, [`TaskError::Refused`] when the server does.
pub async fn start_with_aware(
    api: &impl Api,
    facts: &Facts,
    departments: &[String],
    dir: &Path,
    id: &str,
    aware: Option<&Path>,
) -> Result<Task, TaskError> {
    if !valid_task_id(id) {
        return Err(TaskError::Invalid("task id is invalid"));
    }
    let (task, view) = find_task(api, facts, departments, id).await?;
    let verdict = &view.readiness[&task.id];
    if verdict.state != ReadyState::Ready {
        return Err(TaskError::NotReady(verdict.missing.clone()));
    }
    let already_mine = !facts.me.is_empty() && task.holder.as_deref() == Some(facts.me.as_str());
    let task = if already_mine {
        task
    } else {
        let answer: serde_json::Value =
            api::post_json(api, "/api/machine/claim", &serde_json::json!({ "id": id }))
                .await
                .map_err(refusal)?;
        if answer.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
            let why = answer
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("refused");
            return Err(TaskError::Refused(why.to_owned()));
        }
        answer
            .get("task")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or(task)
    };
    let ctx = gather_context(api, &task, aware).await;
    fs::write(dir.join(SEED_FILE), readiness::seed_text_with(&task, &ctx))
        .map_err(|e| TaskError::Local(e.to_string()))?;
    Ok(task)
}

/// POSTs to `release` or `done` and treats `{ ok: false }` as a refusal.
async fn post_simple(api: &impl Api, path: &str, body: serde_json::Value) -> Result<(), TaskError> {
    let answer: serde_json::Value = api::post_json(api, path, &body).await.map_err(refusal)?;
    if answer.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        let why = answer
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("refused");
        return Err(TaskError::Refused(why.to_owned()));
    }
    Ok(())
}

/// Gives the task back.
///
/// # Errors
/// On a bad id or a refused call.
pub async fn release_with(api: &impl Api, id: &str) -> Result<(), TaskError> {
    if !valid_task_id(id) {
        return Err(TaskError::Invalid("task id is invalid"));
    }
    post_simple(api, "/api/machine/release", serde_json::json!({ "id": id })).await
}

/// Hands the task to review with a summary.
///
/// # Errors
/// On a bad id, an empty or oversized summary, or a refused call.
pub async fn finish_with(api: &impl Api, id: &str, summary: &str) -> Result<(), TaskError> {
    if !valid_task_id(id) {
        return Err(TaskError::Invalid("task id is invalid"));
    }
    let summary = summary.trim();
    if summary.is_empty() || summary.chars().count() > 4000 {
        return Err(TaskError::Invalid("summary must be 1 to 4000 characters"));
    }
    post_simple(
        api,
        "/api/machine/done",
        serde_json::json!({ "id": id, "summary": summary }),
    )
    .await
}

/// Posts the run receipt: kun comments it on the issue and moves the task to review. Only the
/// holder may; the token stays inside the HTTP edge.
///
/// # Errors
/// [`TaskError::Invalid`] when the receipt breaks the schema (nothing is sent),
/// [`TaskError::Refused`] on a 409.
pub async fn receipt_with(api: &impl Api, receipt: &Receipt) -> Result<(), TaskError> {
    receipt.validate().map_err(TaskError::Invalid)?;
    let answer = api::post_receipt(api, receipt).await.map_err(refusal)?;
    if answer.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        let why = answer
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("refused");
        return Err(TaskError::Refused(why.to_owned()));
    }
    Ok(())
}

/// The formula and the stage an issue was filled from, best effort: `Task.formula` or the kun block's
/// `formula=`, and the block's `stage=`. A failed lookup only means a thinner receipt.
pub async fn task_labels(api: &impl Api, id: &str) -> (Option<String>, Option<String>) {
    match api::get_json::<TaskDetail>(api, &format!("/api/machine/task?id={}", id.replace('#', "%23"))).await {
        Ok(d) => (d.task.formula.or(d.task.kun.formula), d.task.kun.stage),
        Err(_) => (None, None),
    }
}

/// The driver this machine runs (the profile's when it fixes one) and the milliseconds since the
/// task was started here (the seed's age); both best effort.
/// The longest duration the kun API accepts on a run receipt (`receiptSchema.durationMs`).
const MAX_RECEIPT_MS: u64 = 7 * 24 * 3600 * 1000;

fn run_facts() -> (String, u64) {
    let driver = match (profile::current(), settings::load()) {
        (Ok(r), _) if r.driver_locked => r.profile.driver,
        (_, Ok(s)) => s.driver,
        (Ok(r), Err(_)) => r.profile.driver,
        _ => crate::driver::DriverKind::Claude,
    };
    let started = work_dir()
        .ok()
        .and_then(|d| fs::metadata(d.join(SEED_FILE)).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
        // The server refuses a receipt longer than a week; an order may be held longer.
        .min(MAX_RECEIPT_MS);
    (driver.binary().to_owned(), started)
}

/// Forgets the held task and its seed, when they are `id`'s.
fn clear_held(id: &str) {
    if host::held_task().as_deref() == Some(id) {
        if let Err(e) = host::set_held_task(None) {
            tracing::warn!("could not clear the held task: {e}");
        }
        if let Ok(dir) = work_dir() {
            let seed = dir.join(SEED_FILE);
            if fs::read_to_string(&seed).is_ok_and(|t| readiness::seed_repos(&t, id).is_some()) {
                let _ = fs::remove_file(seed);
            }
        }
    }
}

async fn facts_now() -> Result<Facts, TaskError> {
    tokio::task::spawn_blocking(|| gather_facts(&RealHost))
        .await
        .map_err(|e| TaskError::Local(e.to_string()))
}

/// `tasks` — the department's queue and board, each task with its gate verdict.
#[tauri::command]
async fn tasks(department: String) -> Result<TasksView, TaskError> {
    if !pack::valid_name(&department) {
        return Err(TaskError::Invalid("department name is invalid"));
    }
    let facts = facts_now().await?;
    view_with(&HttpApi::from_process()?, &facts, &department).await
}

/// `start` — re-checks the gate, claims, writes `TASK.md`. The chat opens with `?task=<id>`.
#[tauri::command]
async fn start(id: String) -> Result<Task, TaskError> {
    let facts = facts_now().await?;
    let dir = tokio::task::spawn_blocking(work_dir)
        .await
        .map_err(|e| TaskError::Local(e.to_string()))??;
    let task = start_with_aware(
        &HttpApi::from_process()?,
        &facts,
        &departments_to_search(),
        &dir,
        &id,
        aware_path().as_deref(),
    )
    .await?;
    host::set_held_task(Some(&task.id)).map_err(TaskError::Local)?;
    Ok(task)
}

/// `release` — gives the task back.
#[tauri::command]
async fn release(id: String) -> Result<(), TaskError> {
    release_with(&HttpApi::from_process()?, &id).await?;
    clear_held(&id);
    Ok(())
}

/// `done` — sends the task to review with a summary; the CEO closes the issue.
#[tauri::command]
async fn done(id: String, summary: String) -> Result<(), TaskError> {
    finish_with(&HttpApi::from_process()?, &id, &summary).await?;
    clear_held(&id);
    Ok(())
}

/// `post_receipt` — sends the run receipt; the task goes to review. The chat keeps the task held
/// until `done` or `release`.
#[tauri::command]
async fn post_receipt(receipt: Receipt) -> Result<(), TaskError> {
    receipt_with(&HttpApi::from_process()?, &receipt).await
}

/// `submit_work` — uploads the chosen files to the CDN through kun's presigned links (progress goes down
/// `channel`), then posts the run receipt with their links and sha256 hashes; the task goes to review.
/// `summary` is the person's one line; the receipt's model is `human`. Returns the public links. A refused
/// upload or receipt comes back as kun's own reason.
#[tauri::command]
async fn submit_work(
    id: String,
    paths: Vec<String>,
    summary: String,
    next: Option<String>,
    channel: Channel<upload::SubmitEvent>,
) -> Result<Vec<String>, TaskError> {
    if !valid_task_id(&id) {
        return Err(TaskError::Invalid("task id is invalid"));
    }
    let home = dirs::home_dir().ok_or_else(|| TaskError::Local("could not find the home folder".into()))?;
    let picked = upload::pick_files(&paths, &upload::allowed_roots(&home))?;
    let api = HttpApi::from_process()?;
    let (formula, stage) = task_labels(&api, &id).await;
    let (driver, duration_ms) = tokio::task::spawn_blocking(run_facts)
        .await
        .map_err(|e| TaskError::Local(e.to_string()))?;
    let notes = upload::Notes { driver, model: "human".to_owned(), duration_ms, summary, next, formula, stage };
    let on_event: Arc<dyn Fn(upload::SubmitEvent) + Send + Sync> = Arc::new(move |e| {
        let _ = channel.send(e);
    });
    upload::submit_with(&api, &upload::put_client()?, &id, &picked, &notes, on_event).await
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tasks")
        .invoke_handler(tauri::generate_handler![tasks, start, release, done, post_receipt, submit_work])
        .build()
}

/// The repo folders a task's run opens, among the clones on this machine: used by chat.
#[must_use]
pub fn task_repo_dirs(host: &impl Host, names: &[String]) -> Vec<PathBuf> {
    host.repo_paths()
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| names.iter().any(|r| r.as_str() == n.to_string_lossy()))
        })
        .collect()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
