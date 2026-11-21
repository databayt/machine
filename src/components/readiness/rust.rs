//! readiness — Machine as the control room over this computer's configuration.
//!
//! kun owns every check and every fix (`<kun>/.claude/machine/readiness.json` and
//! `<kun>/.claude/skills/provision/scripts/provision.mjs`). This feature loads the catalog, runs the
//! script, streams its NDJSON to the page, and does the three small fixes that are not the runner's
//! (open Terminal, open a link, message an owner). Nothing here probes the computer itself.
//!
//! Commands: `readiness_state`, `readiness_run`, `readiness_cancel`, `readiness_fix`,
//! `readiness_uninstall`, `readiness_uninstall_terminal`, `heal_with_agent`, `readiness_agents_tools`.

use std::path::PathBuf;

use tauri::{
    Runtime,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
};

#[path = "agents.rs"]
mod agents;
#[path = "catalog.rs"]
mod catalog;
#[path = "fix.rs"]
mod fix;
#[path = "heal.rs"]
mod heal;
#[path = "model.rs"]
pub mod model;
#[path = "run.rs"]
mod run;
#[path = "uninstall.rs"]
mod uninstall;

use catalog::Scope;
pub use model::ReadinessBeat;
use model::{
    AgentToolRow, Catalog, FixKind, FixOutcome, Mode, ReadinessError, ReadinessState,
    ReadinessView, RunEvent, RunSummary, UninstallOutcome,
};

/// The kun login readiness requests go to.
const OWNER: &str = "abdout";

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn ledger_path() -> Option<PathBuf> {
    crate::pack::machine_dir()
        .ok()
        .map(|d| d.join("heal-attempts.json"))
}

fn load_all() -> (Option<Catalog>, Option<ReadinessState>) {
    let catalog = catalog::load(&crate::engine::kun_dir()).ok();
    (catalog, catalog::load_state(&crate::engine::claude_dir()))
}

/// The heartbeat's readiness field, from the state file; `None` before the first run.
#[must_use]
pub fn beat() -> Option<ReadinessBeat> {
    catalog::load_state(&crate::engine::claude_dir()).map(|s| catalog::beat(&s))
}

/// Failing catalog steps the health pass may fix by itself (auto, under the attempt cap).
#[must_use]
pub fn failing_auto_steps() -> Vec<String> {
    let (Ok(catalog), Some(state)) = (
        catalog::load(&crate::engine::kun_dir()),
        catalog::load_state(&crate::engine::claude_dir()),
    ) else {
        return Vec::new();
    };
    let ledger = ledger_path().map(|p| heal::load(&p)).unwrap_or_default();
    heal::candidates(&catalog, &Scope::current(), &state, &ledger, now_secs())
}

/// Fixes one step with `provision.mjs --complete --only <id>`, counting the attempt. Blocking: the
/// health pass calls it from its worker thread.
///
/// # Errors
/// When the step id is bad, a run is already going, or the step is still failing afterwards.
pub fn heal_step(id: &str) -> Result<(), String> {
    if !catalog::valid_id(id) {
        return Err(ReadinessError::BadId(id.to_owned()).to_string());
    }
    let _guard = run::acquire().map_err(|e| e.to_string())?;
    if let Some(path) = ledger_path() {
        heal::record(&path, id, now_secs()).map_err(|e| e.to_string())?;
    }
    let only = [id.to_owned()];
    let summary = tokio::runtime::Handle::try_current()
        .map_err(|e| e.to_string())?
        .block_on(run::provision(
            &crate::engine::kun_dir(),
            Mode::Complete,
            &only,
            &Scope::current(),
            &mut |_| {},
        ))
        .map_err(|e| e.to_string())?;
    if summary.failing.iter().any(|f| f == id) {
        Err("still failing after the fix".to_owned())
    } else {
        Ok(())
    }
}

/// Opens Terminal.app running `line`; the config feature uses it to load secrets.
///
/// # Errors
/// Off macOS, or when `osascript` fails.
pub async fn run_in_terminal(line: &str) -> Result<(), String> {
    fix::open_terminal(line).await.map_err(|e| e.to_string())
}

/// Single-quotes text for a POSIX shell.
#[must_use]
pub fn shell_quote(text: &str) -> String {
    fix::shell_quote(text)
}

/// `readiness_state` — the catalog's steps for this computer with where each stands.
#[tauri::command]
async fn readiness_state() -> Result<ReadinessView, ReadinessError> {
    tokio::task::spawn_blocking(|| {
        let (catalog, state) = load_all();
        catalog::view(catalog.as_ref(), &Scope::current(), state.as_ref())
    })
    .await
    .map_err(|e| ReadinessError::Other(e.to_string()))
}

/// `readiness_run` — runs the provision script in `mode`, streaming its events over `channel`.
/// One run at a time.
#[tauri::command]
async fn readiness_run(
    mode: Mode,
    only: Option<Vec<String>>,
    channel: Channel<RunEvent>,
) -> Result<RunSummary, ReadinessError> {
    if mode == Mode::Uninstall {
        return Err(ReadinessError::Invalid(
            "uninstalling goes through readiness_uninstall",
        ));
    }
    let _guard = run::acquire()?;
    let scope = tokio::task::spawn_blocking(Scope::current)
        .await
        .map_err(|e| ReadinessError::Other(e.to_string()))?;
    run::provision(
        &crate::engine::kun_dir(),
        mode,
        &only.unwrap_or_default(),
        &scope,
        &mut |e| {
            let _ = channel.send(e);
        },
    )
    .await
}

/// `readiness_uninstall` — removes the steps in `ids` with `provision.mjs --uninstall --only …`,
/// streaming its events, and sorts what happened per step. A terminal or keystone uninstall is
/// never run: it comes back as a handoff for `readiness_uninstall_terminal`. One run at a time.
#[tauri::command]
async fn readiness_uninstall(
    ids: Vec<String>,
    channel: Channel<RunEvent>,
) -> Result<UninstallOutcome, ReadinessError> {
    if ids.is_empty() {
        return Err(ReadinessError::Invalid("nothing to uninstall"));
    }
    if let Some(bad) = ids.iter().find(|i| !catalog::valid_id(i)) {
        return Err(ReadinessError::BadId(bad.clone()));
    }
    let mut only: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !only.contains(&id) {
            only.push(id);
        }
    }
    let _guard = run::acquire()?;
    let scope = tokio::task::spawn_blocking(Scope::current)
        .await
        .map_err(|e| ReadinessError::Other(e.to_string()))?;
    let kun = crate::engine::kun_dir();
    let mut seen: Vec<RunEvent> = Vec::new();
    let summary = run::provision(&kun, Mode::Uninstall, &only, &scope, &mut |e| {
        seen.push(e.clone());
        let _ = channel.send(e);
    })
    .await?;
    let catalog = catalog::load(&kun).ok();
    Ok(uninstall::outcome(
        &only,
        &seen,
        catalog.as_ref(),
        summary.cancelled,
    ))
}

/// `readiness_uninstall_terminal` — opens Terminal on one handoff. The command is the catalog's,
/// read from disk here; the webview only names the step.
#[tauri::command]
async fn readiness_uninstall_terminal(id: String) -> Result<(), ReadinessError> {
    if !catalog::valid_id(&id) {
        return Err(ReadinessError::BadId(id));
    }
    let kun = crate::engine::kun_dir();
    let catalog = catalog::load(&kun)?;
    let step = catalog
        .steps
        .iter()
        .find(|s| s.id == id)
        .ok_or(ReadinessError::UnknownStep(id))?;
    let line = fix::uninstall_line(&kun, step)?;
    fix::open_terminal(&line).await
}

/// `readiness_cancel` — stops the run that is going; false when none is.
#[tauri::command]
fn readiness_cancel() -> bool {
    run::cancel()
}

/// `readiness_fix` — does what the step's `fix.kind` says: the runner for `auto`, Terminal for
/// `terminal`, the opener for `link`, a message to the owner for `request`.
#[tauri::command]
async fn readiness_fix(
    id: String,
    channel: Channel<RunEvent>,
) -> Result<FixOutcome, ReadinessError> {
    if !catalog::valid_id(&id) {
        return Err(ReadinessError::BadId(id));
    }
    let kun = crate::engine::kun_dir();
    let catalog = catalog::load(&kun)?;
    let step = catalog
        .steps
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| ReadinessError::UnknownStep(id.clone()))?;
    let kind = step.fix.as_ref().map_or(FixKind::Auto, |f| f.kind);
    let done = |message: Option<String>| FixOutcome {
        kind,
        ok: true,
        message,
    };
    match fix::plan(step, &kun)? {
        fix::Action::Provision(only) => {
            let _guard = run::acquire()?;
            let scope = tokio::task::spawn_blocking(Scope::current)
                .await
                .map_err(|e| ReadinessError::Other(e.to_string()))?;
            let summary = run::provision(
                &kun,
                Mode::Complete,
                std::slice::from_ref(&only),
                &scope,
                &mut |e| {
                    let _ = channel.send(e);
                },
            )
            .await?;
            let failed = summary.failing.contains(&only);
            Ok(FixOutcome {
                kind,
                ok: !failed && !summary.cancelled,
                message: failed.then(|| "still failing after the fix".to_owned()),
            })
        }
        fix::Action::Terminal(line) => {
            fix::open_terminal(&line).await?;
            Ok(done(step.fix.as_ref().and_then(|f| f.message.clone())))
        }
        fix::Action::Link(url) => {
            fix::open_link(&url).await?;
            Ok(done(None))
        }
        fix::Action::Request(text) => {
            let api = crate::network::api::HttpApi::from_process()
                .map_err(|e| ReadinessError::Other(e.to_string()))?;
            crate::network::send_with(
                &api,
                OWNER,
                crate::network::model::MessageKind::Note,
                &text,
                None,
            )
            .await
            .map_err(|e| ReadinessError::Other(e.to_string()))?;
            Ok(done(Some(format!("sent to {OWNER}"))))
        }
    }
}

/// `heal_with_agent` — starts the default driver (Claude) in the kun folder with the provision
/// skill, streaming the driver's events. Shares the one-run lock with `readiness_run`.
#[tauri::command]
async fn heal_with_agent(
    ids: Vec<String>,
    channel: Channel<crate::driver::Event>,
) -> Result<(), ReadinessError> {
    if ids.is_empty() {
        return Err(ReadinessError::Invalid("nothing to heal"));
    }
    if let Some(bad) = ids.iter().find(|i| !catalog::valid_id(i)) {
        return Err(ReadinessError::BadId(bad.clone()));
    }
    let _guard = run::acquire()?;
    let job = crate::driver::Job {
        job_id: format!("heal-{}", now_secs()),
        driver: crate::driver::DriverKind::Claude,
        prompt: format!("Use the provision skill to heal: {}", ids.join(", ")),
        cwd: crate::engine::kun_dir().display().to_string(),
        session: None,
        timeout_secs: Some(1800),
        extra_args: Vec::new(),
        env: Vec::new(),
        permission_mode: crate::driver::PermissionMode::Auto,
    };
    crate::driver::run(&job, |e| {
        let _ = channel.send(e);
    })
    .await
    .map_err(|e| ReadinessError::Other(e.to_string()))
}

/// `readiness_agents_tools` — drivers and tools merged with their catalog steps.
#[tauri::command]
async fn readiness_agents_tools() -> Result<Vec<AgentToolRow>, ReadinessError> {
    let tools = crate::tools::probe().await;
    tokio::task::spawn_blocking(move || {
        let (catalog, state) = load_all();
        agents::merge(
            &crate::driver::detect(),
            &tools,
            catalog.as_ref(),
            state.as_ref(),
        )
    })
    .await
    .map_err(|e| ReadinessError::Other(e.to_string()))
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("readiness")
        .invoke_handler(tauri::generate_handler![
            readiness_state,
            readiness_run,
            readiness_cancel,
            readiness_fix,
            readiness_uninstall,
            readiness_uninstall_terminal,
            heal_with_agent,
            readiness_agents_tools
        ])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
