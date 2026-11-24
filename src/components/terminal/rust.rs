//! terminal — shells (and the agent launchers inside them) living in Rust, shown by any view.
//!
//! A session is a login interactive zsh in a pty. "+" opens a plain `Local` shell; a launcher
//! (an id in kun's `launchers.json`: `c`, `a`, `o`, `h`, `f`, …) opens a shell that runs that
//! launcher first and falls back to the prompt when it quits. The page never sends a command
//! string: Rust looks the program up, quotes it, and starts it through `/bin/zsh -l -i` so `PATH`
//! and `~/.claude/.env` load even when the app was launched from Finder.
//!
//! Sessions are a registry here, not React state: id, name, lane, cwd, created, alive, and a
//! bounded scrollback ring (oldest bytes dropped). Leaving the page leaves the ptys running, and a
//! second window attaches to the same ones: `terminal_attach` returns the scrollback and the byte
//! count it covers, and `terminal://data/<id>` chunks carry their own end offset so a view can
//! replay, then follow live without a gap or a repeat.
//!
//! Commands: `terminal_lanes`, `terminal_list`, `terminal_open`, `terminal_attach`,
//! `terminal_write`, `terminal_resize`, `terminal_close`, `terminal_close_others`,
//! `terminal_rename`, `terminal_set_active`, `terminal_fullscreen`. Events: `terminal://sessions` (the list, whenever it changes),
//! `terminal://data/<id>` ([`TerminalChunk`]), `terminal://exit/<id>` (exit code or null).
//! The pure part (`plan`, `shell_quote`, [`Registry`], [`Ring`]) is tested in `rust_test.rs`.

use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
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
const MAX_NAME_CHARS: usize = 40;
/// Scrollback kept per session.
const SCROLLBACK_BYTES: usize = 512 * 1024;

/// One launcher the picker can start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "terminal/types.ts")]
pub struct TerminalLane {
    pub id: String,
    pub name: String,
    /// The program is on this computer.
    pub installed: bool,
}

/// One session as the views see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "terminal/types.ts")]
pub struct SessionInfo {
    pub id: String,
    /// What the tab says: the name the person gave, else the default (`Local`, `Local (2)`, a launcher's name).
    pub name: String,
    /// `local`, or the launcher's id.
    pub lane: String,
    pub cwd: String,
    /// Unix milliseconds.
    #[ts(type = "number")]
    pub created_at: u64,
    /// The shell is still running.
    pub alive: bool,
}

/// The sessions in tab order, and which one is in front.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "terminal/types.ts")]
pub struct TerminalList {
    pub sessions: Vec<SessionInfo>,
    pub active: Option<String>,
}

/// What `terminal_attach` replays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "terminal/types.ts")]
pub struct Attached {
    /// The scrollback, base64.
    pub data: String,
    /// Total bytes the session has produced up to the end of `data`.
    #[ts(type = "number")]
    pub total: u64,
}

/// One live chunk of output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "terminal/types.ts")]
pub struct TerminalChunk {
    /// Total bytes the session has produced after this chunk (so its start is `end - len`).
    #[ts(type = "number")]
    pub end: u64,
    /// The bytes, base64.
    pub data: String,
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

/// What to start: the zsh argv, the folder, and how the tab is named by default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// `local`, or the launcher id.
    pub lane: String,
    /// The default tab name, before `Local (2)`-style numbering.
    pub base: String,
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

const ZSH: [&str; 3] = ["/bin/zsh", "-l", "-i"];

/// Lane → the argv that runs it. `None` or `local` is the person's own shell; anything else must
/// be a launcher id, run inside a shell so quitting the agent leaves the prompt. Pure apart from
/// folder checks.
///
/// # Errors
/// When the lane is not in `specs`, anything names `fable`, or the folder is not one.
pub(crate) fn plan(lane: Option<&str>, specs: &[LauncherSpec], cwd: Option<&str>, home: &Path) -> Result<Plan, TerminalError> {
    let lane = lane.map(str::trim).filter(|l| !l.is_empty() && *l != "local");
    let spec = match lane {
        None => None,
        Some(l) => Some(
            specs
                .iter()
                .find(|s| s.id == l)
                .ok_or_else(|| TerminalError::Unknown(l.chars().take(32).collect()))?,
        ),
    };
    if let (Some(l), Some(spec)) = (lane, spec)
        && (fable(l) || fable(&spec.bin) || spec.args.iter().any(|a| fable(a)))
    {
        return Err(TerminalError::Refused("model"));
    }
    if cwd.is_some_and(fable) {
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
    let mut argv: Vec<String> = ZSH.map(String::from).to_vec();
    let Some(spec) = spec else {
        return Ok(Plan { argv, cwd, lane: "local".into(), base: "Local".into() });
    };
    let run = std::iter::once(shell_quote(&spec.bin))
        .chain(spec.args.iter().map(|a| shell_quote(a)))
        .collect::<Vec<_>>()
        .join(" ");
    argv.push("-c".into());
    argv.push(format!("{run}; exec {}", ZSH.join(" ")));
    Ok(Plan { argv, cwd, lane: spec.id.clone(), base: spec.name.clone() })
}

fn clamp(n: u16) -> u16 {
    n.clamp(1, MAX_DIM)
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: clamp(rows), cols: clamp(cols), pixel_width: 0, pixel_height: 0 }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// A bounded byte buffer: the newest `cap` bytes, plus how many bytes ever went through.
#[derive(Debug)]
pub(crate) struct Ring {
    buf: VecDeque<u8>,
    total: u64,
    cap: usize,
}

impl Ring {
    pub(crate) fn new(cap: usize) -> Self {
        Self { buf: VecDeque::new(), total: 0, cap }
    }

    /// Appends, dropping the oldest bytes past the cap. Returns the new total.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> u64 {
        self.total += bytes.len() as u64;
        if bytes.len() >= self.cap {
            self.buf.clear();
            self.buf.extend(&bytes[bytes.len() - self.cap..]);
        } else {
            self.buf.extend(bytes);
            let over = self.buf.len().saturating_sub(self.cap);
            if over > 0 {
                self.buf.drain(..over);
            }
        }
        self.total
    }

    pub(crate) fn snapshot(&self) -> Vec<u8> {
        self.buf.iter().copied().collect()
    }

    pub(crate) fn total(&self) -> u64 {
        self.total
    }
}

/// The pty ends of a live session.
struct Io {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl Io {
    fn kill(&mut self) {
        let _ = self.killer.kill();
    }
}

struct Session {
    info: SessionInfo,
    default_name: String,
    ring: Ring,
    io: Option<Io>,
}

/// The open sessions, in tab order. No pty knowledge beyond holding the ends: all of it is
/// testable with `io: None`.
#[derive(Default)]
pub(crate) struct Registry {
    order: Vec<String>,
    by_id: HashMap<String, Session>,
    active: Option<String>,
    next: u64,
}

/// The tab name: trimmed, control characters dropped, at most 40 characters; empty means "use the default".
pub(crate) fn clean_name(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(MAX_NAME_CHARS).collect::<String>().trim().to_owned()
}

impl Registry {
    pub(crate) fn len(&self) -> usize {
        self.by_id.len()
    }

    pub(crate) fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// A free id of our own (`t1`, `t2`, …).
    pub(crate) fn fresh_id(&mut self) -> String {
        loop {
            self.next += 1;
            let id = format!("t{}", self.next);
            if !self.contains(&id) {
                return id;
            }
        }
    }

    /// `base`, else `base (2)`, `base (3)`… the first not used by an open tab.
    pub(crate) fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| self.by_id.values().any(|s| s.info.name == n);
        if !taken(base) {
            return base.to_owned();
        }
        (2..=10_000).map(|n| format!("{base} ({n})")).find(|n| !taken(n)).unwrap_or_else(|| base.to_owned())
    }

    /// Adds a session at the end and makes it the active one. The caller has checked the cap and the id.
    fn add(&mut self, id: String, plan: &Plan, io: Option<Io>) -> SessionInfo {
        let name = self.unique_name(&plan.base);
        let info = SessionInfo {
            id: id.clone(),
            name: name.clone(),
            lane: plan.lane.clone(),
            cwd: plan.cwd.to_string_lossy().into_owned(),
            created_at: now_ms(),
            alive: io.is_some(),
        };
        self.by_id.insert(id.clone(), Session { info: info.clone(), default_name: name, ring: Ring::new(SCROLLBACK_BYTES), io });
        self.order.push(id.clone());
        self.active = Some(id);
        info
    }

    /// Renames; an empty name falls back to the default one.
    pub(crate) fn rename(&mut self, id: &str, raw: &str) -> Result<(), TerminalError> {
        let s = self.by_id.get_mut(id).ok_or(TerminalError::Gone)?;
        let name = clean_name(raw);
        s.info.name = if name.is_empty() { s.default_name.clone() } else { name };
        Ok(())
    }

    pub(crate) fn set_active(&mut self, id: &str) -> Result<(), TerminalError> {
        if !self.contains(id) {
            return Err(TerminalError::Gone);
        }
        self.active = Some(id.to_owned());
        Ok(())
    }

    /// Removes one. If it was in front, the tab to its left (else the new first) takes over.
    fn remove(&mut self, id: &str) -> Option<Session> {
        let at = self.order.iter().position(|o| o == id)?;
        self.order.remove(at);
        let gone = self.by_id.remove(id);
        if self.active.as_deref() == Some(id) {
            self.active = self.order.get(at.saturating_sub(1)).cloned();
        }
        gone
    }

    /// Removes every session but `keep`, which becomes the active one.
    fn remove_others(&mut self, keep: &str) -> Result<Vec<Session>, TerminalError> {
        if !self.contains(keep) {
            return Err(TerminalError::Gone);
        }
        let others: Vec<String> = self.order.iter().filter(|o| *o != keep).cloned().collect();
        let gone = others.iter().filter_map(|o| self.by_id.remove(o)).collect();
        self.order = vec![keep.to_owned()];
        self.active = Some(keep.to_owned());
        Ok(gone)
    }

    pub(crate) fn list(&self) -> TerminalList {
        TerminalList {
            sessions: self.order.iter().filter_map(|id| self.by_id.get(id)).map(|s| s.info.clone()).collect(),
            active: self.active.clone(),
        }
    }

    /// Output arrived: into the ring. Returns the new total, or `None` when the session is gone.
    pub(crate) fn push(&mut self, id: &str, bytes: &[u8]) -> Option<u64> {
        self.by_id.get_mut(id).map(|s| s.ring.push(bytes))
    }

    /// The scrollback and the byte count it covers.
    pub(crate) fn attach(&self, id: &str) -> Result<(Vec<u8>, u64), TerminalError> {
        let s = self.by_id.get(id).ok_or(TerminalError::Gone)?;
        Ok((s.ring.snapshot(), s.ring.total()))
    }

    fn mark_dead(&mut self, id: &str) {
        if let Some(s) = self.by_id.get_mut(id) {
            s.info.alive = false;
            s.io = None;
        }
    }

    fn drain_all(&mut self) -> Vec<Session> {
        self.order.clear();
        self.active = None;
        self.by_id.drain().map(|(_, s)| s).collect()
    }
}

/// The app's terminals.
#[derive(Default)]
pub struct Terminals {
    reg: Mutex<Registry>,
}

impl Terminals {
    fn reg(&self) -> MutexGuard<'_, Registry> {
        self.reg.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn close_all(&self) {
        let all = self.reg().drain_all();
        kill_all(all);
    }
}

/// Tells every window the list changed.
fn notify<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<Terminals>() else { return };
    let list = state.reg().list();
    let _ = app.emit("terminal://sessions", &list);
}

fn kill_all(sessions: Vec<Session>) {
    for mut s in sessions {
        if let Some(io) = s.io.as_mut() {
            io.kill();
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

/// `terminal_list` — the sessions in tab order and the active id.
#[tauri::command]
async fn terminal_list(state: State<'_, Terminals>) -> Result<TerminalList, TerminalError> {
    Ok(state.reg().list())
}

/// `terminal_open` — starts a shell (lane `local` or none) or a launcher inside a shell, makes it
/// the active session and returns its id. The page may pick the `id`; otherwise one is made.
#[tauri::command]
async fn terminal_open<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Terminals>,
    lane: Option<String>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    id: Option<String>,
) -> Result<String, TerminalError> {
    if id.as_deref().is_some_and(|i| !valid_id(i)) {
        return Err(TerminalError::Refused("terminal id"));
    }
    let specs = if lane.as_deref().map(str::trim).is_none_or(|l| l.is_empty() || l == "local") {
        Vec::new()
    } else {
        launcher_specs().map_err(TerminalError::NoCatalog)?
    };
    let home = dirs::home_dir().unwrap_or_default();
    let plan = plan(lane.as_deref(), &specs, cwd.as_deref(), &home)?;
    {
        let reg = state.reg();
        if reg.len() >= MAX_TERMINALS {
            return Err(TerminalError::TooMany);
        }
        if id.as_deref().is_some_and(|i| reg.contains(i)) {
            return Err(TerminalError::Refused("terminal id in use"));
        }
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

    let id = {
        let mut reg = state.reg();
        // Re-checked under the same lock that inserts: two quick opens cannot both pass.
        let clash = reg.len() >= MAX_TERMINALS || id.as_deref().is_some_and(|i| reg.contains(i));
        if clash {
            let mut io = Io { master: pty.master, writer, killer };
            io.kill();
            return Err(TerminalError::TooMany);
        }
        let id = id.unwrap_or_else(|| reg.fresh_id());
        reg.add(id.clone(), &plan, Some(Io { master: pty.master, writer, killer }));
        id
    };

    let pump_id = id.clone();
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            let end = handle.try_state::<Terminals>().and_then(|t| t.reg().push(&pump_id, &buf[..n]));
            let Some(end) = end else { break };
            let chunk = TerminalChunk { end, data: STANDARD.encode(&buf[..n]) };
            let _ = handle.emit(&format!("terminal://data/{pump_id}"), chunk);
        }
        let code = child.wait().ok().map(|s| i64::from(s.exit_code()));
        if let Some(t) = handle.try_state::<Terminals>() {
            t.reg().mark_dead(&pump_id);
        }
        let _ = handle.emit(&format!("terminal://exit/{pump_id}"), code);
        notify(&handle);
    });
    notify(&app);
    Ok(id)
}

/// `terminal_attach` — the scrollback of a session, to replay before following the live events.
#[tauri::command]
async fn terminal_attach(state: State<'_, Terminals>, id: String) -> Result<Attached, TerminalError> {
    let (bytes, total) = state.reg().attach(&id)?;
    Ok(Attached { data: STANDARD.encode(bytes), total })
}

/// `terminal_write` — keystrokes (or a paste) into the terminal.
#[tauri::command]
async fn terminal_write(state: State<'_, Terminals>, id: String, data: String) -> Result<(), TerminalError> {
    if data.len() > MAX_WRITE_BYTES {
        return Err(TerminalError::Refused("input too large"));
    }
    let mut reg = state.reg();
    let io = reg.by_id.get_mut(&id).and_then(|s| s.io.as_mut()).ok_or(TerminalError::Gone)?;
    io.writer.write_all(data.as_bytes()).and_then(|()| io.writer.flush()).map_err(|e| TerminalError::Pty(e.to_string()))
}

/// `terminal_resize` — the page's grid changed.
#[tauri::command]
async fn terminal_resize(state: State<'_, Terminals>, id: String, cols: u16, rows: u16) -> Result<(), TerminalError> {
    let reg = state.reg();
    let io = reg.by_id.get(&id).and_then(|s| s.io.as_ref()).ok_or(TerminalError::Gone)?;
    io.master.resize(size(cols, rows)).map_err(|e| TerminalError::Pty(e.to_string()))
}

/// `terminal_close` — kills the process and frees the session. Closing a gone one is fine.
#[tauri::command]
async fn terminal_close<R: Runtime>(app: AppHandle<R>, state: State<'_, Terminals>, id: String) -> Result<(), TerminalError> {
    let gone = state.reg().remove(&id);
    kill_all(gone.into_iter().collect());
    notify(&app);
    Ok(())
}

/// `terminal_close_others` — keeps `id` (now the active one), closes the rest.
#[tauri::command]
async fn terminal_close_others<R: Runtime>(app: AppHandle<R>, state: State<'_, Terminals>, id: String) -> Result<(), TerminalError> {
    let gone = state.reg().remove_others(&id)?;
    kill_all(gone);
    notify(&app);
    Ok(())
}

/// `terminal_rename` — the tab name (max 40 characters); empty restores the default.
#[tauri::command]
async fn terminal_rename<R: Runtime>(app: AppHandle<R>, state: State<'_, Terminals>, id: String, name: String) -> Result<(), TerminalError> {
    state.reg().rename(&id, &name)?;
    notify(&app);
    Ok(())
}

/// `terminal_set_active` — the tab in front, in every window.
#[tauri::command]
async fn terminal_set_active<R: Runtime>(app: AppHandle<R>, state: State<'_, Terminals>, id: String) -> Result<(), TerminalError> {
    state.reg().set_active(&id)?;
    notify(&app);
    Ok(())
}

/// `terminal_fullscreen` — the main window goes native full screen, or leaves it.
#[tauri::command]
async fn terminal_fullscreen<R: Runtime>(app: AppHandle<R>, on: bool) -> Result<(), TerminalError> {
    if let Some(w) = app.get_webview_window("main") {
        w.set_fullscreen(on).map_err(|e| TerminalError::Pty(e.to_string()))?;
    }
    Ok(())
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("terminal")
        .invoke_handler(tauri::generate_handler![
            terminal_lanes,
            terminal_list,
            terminal_open,
            terminal_attach,
            terminal_write,
            terminal_resize,
            terminal_close,
            terminal_close_others,
            terminal_rename,
            terminal_set_active,
            terminal_fullscreen
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
