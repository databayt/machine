//! terminal — a pty running one of the agent launchers, typed into from the app.
//!
//! The lane is an id in kun's `launchers.json` (`c`, `a`, `o`, `h`, `f`, …), never a command the
//! page sends: Rust looks the program and its arguments up itself, quotes them, and starts them
//! through a login interactive zsh so `PATH` and `~/.claude/.env` load even when the app was
//! launched from Finder. Output streams to the page as events, input comes back as commands.
//! The pure part (`plan`, `shell_quote`) is tested in `rust_test.rs`.
//!
//! Commands: `terminal_lanes`, `terminal_open`, `terminal_write`, `terminal_resize`,
//! `terminal_close`. Events: `terminal://data/<id>` (base64 bytes), `terminal://exit/<id>`
//! (exit code or null).

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, Runtime, State,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;

use crate::launchers::{LauncherSpec, launcher_specs};

const MAX_TERMINALS: usize = 12;
const MAX_WRITE_BYTES: usize = 64 * 1024;
const MAX_DIM: u16 = 500;

/// One launcher the picker can start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "terminal/types.ts")]
pub struct TerminalLane {
    pub id: String,
    pub name: String,
    /// The program is on this computer.
    pub installed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("not a launcher: {0}")]
    Unknown(String),
    #[error("refused: {0}")]
    Refused(&'static str),
    #[error("kun has no launchers list here: {0}")]
    NoCatalog(String),
    #[error("too many terminals are open")]
    TooMany,
    #[error("no such terminal")]
    Gone,
    #[error("{0}")]
    Pty(String),
}

impl Serialize for TerminalError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// What to start: the zsh argv and the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
}

/// A page-chosen terminal id: short, plain, safe inside an event name.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `'…'` with embedded quotes closed, escaped and reopened: safe as one zsh word.
pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn fable(s: &str) -> bool {
    s.to_ascii_lowercase().contains("fable")
}

/// `$HOME/kun` when it exists, else `$HOME`.
pub(crate) fn default_cwd(home: &Path) -> PathBuf {
    let kun = home.join("kun");
    if kun.is_dir() { kun } else { home.to_path_buf() }
}

/// Lane id → the argv that runs it. Pure apart from folder checks.
///
/// # Errors
/// When the lane is not in `specs`, anything names `fable`, or the folder is not one.
pub(crate) fn plan(lane: &str, specs: &[LauncherSpec], cwd: Option<&str>, home: &Path) -> Result<Plan, TerminalError> {
    let spec = specs
        .iter()
        .find(|s| s.id == lane)
        .ok_or_else(|| TerminalError::Unknown(lane.chars().take(32).collect()))?;
    if fable(lane)
        || fable(&spec.bin)
        || spec.args.iter().any(|a| fable(a))
        || cwd.is_some_and(fable)
    {
        return Err(TerminalError::Refused("model"));
    }
    let cwd = match cwd.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => {
            let p = PathBuf::from(c);
            if !p.is_absolute() || !p.is_dir() {
                return Err(TerminalError::Refused("cwd is not a folder"));
            }
            p
        }
        None => default_cwd(home),
    };
    let command = std::iter::once(format!("exec {}", shell_quote(&spec.bin)))
        .chain(spec.args.iter().map(|a| shell_quote(a)))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Plan {
        argv: ["/bin/zsh", "-l", "-i", "-c"].map(String::from).into_iter().chain([command]).collect(),
        cwd,
    })
}

fn clamp(n: u16) -> u16 {
    n.clamp(1, MAX_DIM)
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: clamp(rows), cols: clamp(cols), pixel_width: 0, pixel_height: 0 }
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

/// The open terminals, by id.
#[derive(Default)]
pub struct Terminals {
    next: AtomicU64,
    open: Mutex<HashMap<String, Session>>,
}

impl Terminals {
    fn map(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn close(&self, id: &str) {
        if let Some(mut s) = self.map().remove(id) {
            let _ = s.killer.kill();
        }
    }

    fn close_all(&self) {
        let all: Vec<Session> = self.map().drain().map(|(_, s)| s).collect();
        for mut s in all {
            let _ = s.killer.kill();
        }
    }
}

fn installed(spec: &LauncherSpec) -> bool {
    let dirs = crate::driver::search_dirs();
    crate::driver::find_binary(&spec.bin, &dirs).is_some()
        || crate::driver::DriverKind::ALL
            .into_iter()
            .any(|k| k.binary() == spec.bin && crate::driver::find_driver(k, &dirs).is_some())
}

/// `terminal_lanes` — the launchers the picker offers, with whether each program is installed.
#[tauri::command]
async fn terminal_lanes() -> Result<Vec<TerminalLane>, TerminalError> {
    tokio::task::spawn_blocking(|| {
        launcher_specs().map_err(TerminalError::NoCatalog).map(|all| {
            all.iter()
                .map(|s| TerminalLane { id: s.id.clone(), name: s.name.clone(), installed: installed(s) })
                .collect()
        })
    })
    .await
    .map_err(|e| TerminalError::Pty(e.to_string()))?
}

/// `terminal_open` — starts `lane` in a new pty and returns its id. The page may pick the `id`
/// (so it can listen before the first byte is sent); otherwise one is made.
#[tauri::command]
async fn terminal_open<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Terminals>,
    lane: String,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    id: Option<String>,
) -> Result<String, TerminalError> {
    if id.as_deref().is_some_and(|i| !valid_id(i)) {
        return Err(TerminalError::Refused("terminal id"));
    }
    let specs = launcher_specs().map_err(TerminalError::NoCatalog)?;
    let home = dirs::home_dir().unwrap_or_default();
    let plan = plan(&lane, &specs, cwd.as_deref(), &home)?;
    if state.map().len() >= MAX_TERMINALS {
        return Err(TerminalError::TooMany);
    }

    if id.as_deref().is_some_and(|i| state.map().contains_key(i)) {
        return Err(TerminalError::Refused("terminal id in use"));
    }
    let pty = native_pty_system().openpty(size(cols, rows)).map_err(|e| TerminalError::Pty(e.to_string()))?;
    let mut cmd = CommandBuilder::new(&plan.argv[0]);
    cmd.args(&plan.argv[1..]);
    cmd.cwd(&plan.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // A Finder launch has a bare PATH; start from the dirs the CLIs are found in (zsh -l adds its own).
    cmd.env("PATH", crate::driver::run_path(&crate::driver::search_dirs()));
    let mut child = pty.slave.spawn_command(cmd).map_err(|e| TerminalError::Pty(e.to_string()))?;
    drop(pty.slave);
    let killer = child.clone_killer();
    let mut reader = pty.master.try_clone_reader().map_err(|e| TerminalError::Pty(e.to_string()))?;
    let writer = pty.master.take_writer().map_err(|e| TerminalError::Pty(e.to_string()))?;

    let id = id.unwrap_or_else(|| format!("t{}", state.next.fetch_add(1, Ordering::Relaxed) + 1));
    state.map().insert(id.clone(), Session { master: pty.master, writer, killer });

    let pump_id = id.clone();
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            let _ = handle.emit(&format!("terminal://data/{pump_id}"), STANDARD.encode(&buf[..n]));
        }
        let code = child.wait().ok().map(|s| i64::from(s.exit_code()));
        if let Some(t) = handle.try_state::<Terminals>() {
            t.map().remove(&pump_id);
        }
        let _ = handle.emit(&format!("terminal://exit/{pump_id}"), code);
    });
    Ok(id)
}

/// `terminal_write` — keystrokes (or a paste) into the terminal.
#[tauri::command]
async fn terminal_write(state: State<'_, Terminals>, id: String, data: String) -> Result<(), TerminalError> {
    if data.len() > MAX_WRITE_BYTES {
        return Err(TerminalError::Refused("input too large"));
    }
    let mut open = state.map();
    let s = open.get_mut(&id).ok_or(TerminalError::Gone)?;
    s.writer.write_all(data.as_bytes()).and_then(|()| s.writer.flush()).map_err(|e| TerminalError::Pty(e.to_string()))
}

/// `terminal_resize` — the page's grid changed.
#[tauri::command]
async fn terminal_resize(state: State<'_, Terminals>, id: String, cols: u16, rows: u16) -> Result<(), TerminalError> {
    let open = state.map();
    let s = open.get(&id).ok_or(TerminalError::Gone)?;
    s.master.resize(size(cols, rows)).map_err(|e| TerminalError::Pty(e.to_string()))
}

/// `terminal_close` — kills the process and frees the terminal. Closing a gone one is fine.
#[tauri::command]
async fn terminal_close(state: State<'_, Terminals>, id: String) -> Result<(), TerminalError> {
    state.close(&id);
    Ok(())
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("terminal")
        .invoke_handler(tauri::generate_handler![
            terminal_lanes,
            terminal_open,
            terminal_write,
            terminal_resize,
            terminal_close
        ])
        .setup(|app, _api| {
            app.manage(Terminals::default());
            Ok(())
        })
        .on_event(|app, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(t) = app.try_state::<Terminals>()
            {
                t.close_all();
            }
        })
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
