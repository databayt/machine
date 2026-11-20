//! readiness — runs kun's provision script and streams what it prints.
//!
//! machine implements no probe and no fix: `provision.mjs` does, and prints one JSON event per
//! line. This file spawns it (no shell, arguments as a vector), parses the lines, honours a
//! cancel, and kills the child when the run is dropped.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::Notify,
};

use super::{
    catalog::{Scope, valid_id},
    model::{Mode, ReadinessError, RunEvent, RunSummary},
};

/// The longest a provision run may take.
pub(super) const RUN_LIMIT: Duration = Duration::from_secs(30 * 60);

/// `<kun>/.claude/skills/provision/scripts/provision.mjs`.
pub(super) fn script_path(kun: &Path) -> PathBuf {
    kun.join(".claude/skills/provision/scripts/provision.mjs")
}

/// The runner's arguments: the script, the mode, the optional `--only` and the scope.
///
/// # Errors
/// When an id in `only` is not a plain step id.
pub(super) fn build_args(
    script: &Path,
    mode: Mode,
    only: &[String],
    scope: &Scope,
) -> Result<Vec<String>, ReadinessError> {
    if let Some(bad) = only.iter().find(|id| !valid_id(id)) {
        return Err(ReadinessError::BadId(bad.clone()));
    }
    let mut args = vec![script.display().to_string(), mode.flag().to_owned()];
    if !only.is_empty() {
        args.extend(["--only".to_owned(), only.join(",")]);
    }
    if !scope.departments.is_empty() {
        args.extend(["--departments".to_owned(), scope.departments.join(",")]);
    }
    args.extend(["--role".to_owned(), scope.role.clone(), "--json".to_owned()]);
    Ok(args)
}

/// One output line as an event; a blank line is nothing, a line that is not an event is a `Log`.
pub(super) fn parse_line(line: &str) -> Option<RunEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    Some(
        serde_json::from_str(line).unwrap_or_else(|_| RunEvent::Log {
            line: line.chars().take(2000).collect(),
        }),
    )
}

/// Folders a GUI app must look in: Finder-launched apps start with a minimal PATH.
fn node_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".volta/bin"));
        dirs.push(home.join(".local/bin"));
        let mut nvm: Vec<PathBuf> = std::fs::read_dir(home.join(".nvm/versions/node"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path().join("bin"))
            .collect();
        nvm.sort();
        nvm.reverse();
        dirs.extend(nvm);
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

/// The `node` binary, if there is one.
pub(super) fn find_node() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    node_dirs()
        .into_iter()
        .map(|d| d.join(exe))
        .find(|p| p.is_file())
}

static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: LazyLock<Notify> = LazyLock::new(Notify::new);

/// Holds the one-run-at-a-time lock; dropping it frees it.
pub(super) struct Guard(());

impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// Takes the lock.
///
/// # Errors
/// [`ReadinessError::Busy`] while a run or a heal is going.
pub(super) fn acquire() -> Result<Guard, ReadinessError> {
    BUSY.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map(|_| Guard(()))
        .map_err(|_| ReadinessError::Busy)
}

/// Asks the run that holds the lock to stop. True when one was going.
pub(super) fn cancel() -> bool {
    let going = BUSY.load(Ordering::SeqCst);
    if going {
        CANCEL.notify_waiters();
    }
    going
}

/// The run's own cancel handle.
pub(super) fn cancel_handle() -> &'static Notify {
    &CANCEL
}

enum End {
    Finished,
    TimedOut,
    Cancelled,
}

/// Runs `program args…` in `cwd`, calling `emit` for each output line, and summarizes the `done` event.
///
/// # Errors
/// When the process cannot start, or it exits non-zero without ever printing a `done` event.
pub(super) async fn stream(
    program: &Path,
    args: &[String],
    cwd: &Path,
    limit: Duration,
    cancel: &Notify,
    emit: &mut (impl FnMut(RunEvent) + Send),
) -> Result<RunSummary, ReadinessError> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ReadinessError::Spawn("provision", e.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ReadinessError::Spawn("provision", "no stdout".into()))?;
    let stderr = child.stderr.take();
    let err_tail = tokio::spawn(async move {
        let mut text = String::new();
        if let Some(mut e) = stderr {
            let mut buf = Vec::new();
            let _ = (&mut e).take(1 << 20).read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).trim().clone_into(&mut text);
        }
        text
    });

    let mut lines = BufReader::new(stdout).lines();
    let mut summary = RunSummary::default();
    let mut got_done = false;
    let end = tokio::select! {
        r = tokio::time::timeout(limit, async {
            while let Ok(Some(line)) = lines.next_line().await {
                let Some(event) = parse_line(&line) else { continue };
                if let RunEvent::Done { percent, passed, applicable, failing, human } = &event {
                    got_done = true;
                    summary = RunSummary {
                        percent: *percent,
                        passed: *passed,
                        applicable: *applicable,
                        failing: failing.clone(),
                        human: human.clone(),
                        cancelled: false,
                    };
                }
                emit(event);
            }
        }) => if r.is_ok() { End::Finished } else { End::TimedOut },
        () = cancel.notified() => End::Cancelled,
    };

    match end {
        End::Finished => {
            let status = child
                .wait()
                .await
                .map_err(|e| ReadinessError::Spawn("provision", e.to_string()))?;
            if got_done || status.success() {
                return Ok(summary);
            }
            let tail = err_tail.await.unwrap_or_default();
            let code = status
                .code()
                .map_or_else(|| "a signal".to_owned(), |c| c.to_string());
            let what = if tail.is_empty() {
                code
            } else {
                format!("{code}: {}", tail.chars().take(300).collect::<String>())
            };
            let message = ReadinessError::Failed(what);
            emit(RunEvent::Error {
                message: message.to_string(),
            });
            Err(message)
        }
        End::TimedOut => {
            let _ = child.kill().await;
            let message = ReadinessError::Failed("a timeout".into());
            emit(RunEvent::Error {
                message: message.to_string(),
            });
            Err(message)
        }
        End::Cancelled => {
            let _ = child.kill().await;
            summary.cancelled = true;
            Ok(summary)
        }
    }
}

/// Runs the real provision script from the kun checkout.
///
/// # Errors
/// When node or the script is missing, an id is bad, or the run fails (see [`stream`]).
pub(super) async fn provision(
    kun: &Path,
    mode: Mode,
    only: &[String],
    scope: &Scope,
    emit: &mut (impl FnMut(RunEvent) + Send),
) -> Result<RunSummary, ReadinessError> {
    let script = script_path(kun);
    if !script.is_file() {
        return Err(ReadinessError::NoScript(script.display().to_string()));
    }
    let node = find_node().ok_or(ReadinessError::NoNode)?;
    let args = build_args(&script, mode, only, scope)?;
    stream(&node, &args, kun, RUN_LIMIT, cancel_handle(), emit).await
}
