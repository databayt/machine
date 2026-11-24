//! web — the Brainstorm seats: ChatGPT, DeepSeek, GLM and Gemini open inside Kun, on the person's own
//! free accounts, and the rest of the app can ask them a question.
//!
//! **The sites.** Each seat is a native *child webview* of the `main` window (`Window::add_child`,
//! tauri's `unstable` feature), laid over the content box the Brainstorm page reports. It loads the
//! real site; nothing is proxied and no login is read: the person signs in once, inside the app.
//!
//! **Logins survive restarts.** macOS: every seat gets its own persistent `WKWebsiteDataStore`
//! (`data_store_identifier`, macOS 14+), a UUID derived from the seat id, so cookies, local storage and
//! IndexedDB stay on disk under `~/Library/WebKit/org.databayt.machine/WebsiteDataStore/<uuid>`. On macOS
//! 13 and older wry ignores the identifier and every seat shares the app's default persistent store (the
//! logins still survive, they are just not separated). Windows and Linux use `data_directory`:
//! `<machine dir>/web/<seat>` (`~/.kun/machine/web/<seat>`).
//!
//! **How a result comes back.** External sites get no Tauri IPC: the generated capability is local-only
//! (no `remote` urls), so a site calling `invoke` is refused, and navigation is limited to https.
//! Results instead flow the other way: Rust evaluates a script in the page with
//! `Webview::eval_with_callback`, which on macOS is `WKWebView.evaluateJavaScript`'s completion handler
//! (WebView2's `ExecuteScript` on Windows). The script runs synchronously and returns `JSON.stringify`
//! of its answer, so the page never has a channel to write to. A `send` is started (the adapter's
//! promise runs in the page) and then polled every second until the seat is idle and its last answer
//! has held still for three seconds. A script evaluated before the page finishes loading loses its
//! callback (wry queues it), so every evaluation has its own timeout and the loops simply retry.
//!
//! **The adapter** is kun's `<kun>/scripts/web/adapters/<id>.js` (index: `index.json`), a plain
//! idempotent page script that sets `window.__kunWeb`. It is injected as an initialization script
//! (every document) and re-evaluated when the page lost it. Missing adapters fail gracefully: the site
//! still opens for brainstorming, only `web_ask` / `web_status` / `web_transcript` say so.
//!
//! **Guardrails** (mirroring the headless side): one ask at a time per seat, at least 45 s between two
//! asks of a seat, a seat that says `limitHit()` ends the run with an `error` event starting `limit:`,
//! a seat that needs a login ends it with `login:`, and a background ask never raises the window: the
//! seat's webview is created off-screen and hidden unless the Brainstorm page is already showing it.
//!
//! Commands: `web_seats`, `web_open`, `web_hide`, `web_close`, `web_status`, `web_transcript`,
//! `web_ask`. The driver (`DriverKind::Web`, `driver::run`) calls [`run_seat`] with the same events.

// Brand names (ChatGPT, DeepSeek, IndexedDB) are prose here, not code.
#![allow(clippy::doc_markdown)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Position, Runtime, Size, WebviewUrl,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
    webview::{NewWindowResponse, WebviewBuilder},
};
use tokio::sync::oneshot;
use ts_rs::TS;

use crate::driver::Event;

/// At least this long between the starts of two asks on one seat.
pub const MIN_GAP: Duration = Duration::from_secs(45);
/// An answer must hold still this long (and the seat be idle) before it is the answer.
pub const STABLE_FOR: Duration = Duration::from_secs(3);
/// An ask gives up after this long.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(180);
/// A freshly created seat has this long to load, with its adapter ready, before the ask fails.
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_EVERY: Duration = Duration::from_secs(1);
/// One evaluation that never calls back (a page still loading) is abandoned after this.
const EVAL_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_ADAPTER_BYTES: u64 = 256 * 1024;
/// Where a background seat lives while it is hidden: off-screen, so creating it never flashes.
const PARKED: Rect = Rect { x: -5000.0, y: -5000.0, width: 1100.0, height: 760.0 };
/// A browser the sign-in pages of the four sites accept (WKWebView's own user agent has no `Safari/` token).
const SAFARI_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";

// ----------------------------------------------------------------------------- types

/// One row of kun's `adapters/index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "web/types.ts")]
pub struct Seat {
    pub id: String,
    pub name: String,
    pub url: String,
}

/// A seat with what this machine knows about it right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "web/types.ts")]
pub struct SeatInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Its webview exists (visible or parked).
    pub open: bool,
    /// kun ships its adapter, so ask / status / transcript work.
    pub adapter: bool,
}

/// `web_seats`: the seats, or why there are none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "web/types.ts")]
pub struct SeatList {
    pub seats: Vec<SeatInfo>,
    /// kun has no `scripts/web/adapters/index.json` here (or it is damaged): update the kun checkout.
    pub missing: bool,
}

/// Where the Brainstorm page's content box is, in window (logical) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, TS)]
#[ts(export, export_to = "web/types.ts")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// What the adapter says about the page, plus the address it is on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "web/types.ts")]
#[allow(clippy::struct_excessive_bools)] // the adapter's four answers, as the contract names them
pub struct SeatStatus {
    /// The adapter is loaded in the page (false while it loads, or when kun has none).
    pub adapter: bool,
    pub ready: bool,
    pub needs_login: bool,
    /// The limit message when the site says the free quota is used up.
    pub limit: Option<String>,
    pub busy: bool,
    pub url: Option<String>,
}

/// One turn of the conversation on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "web/types.ts")]
pub struct Turn {
    pub role: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("unknown seat `{0}`")]
    UnknownSeat(String),
    #[error("kun has no web adapters here yet; update the kun checkout")]
    NoIndex,
    #[error("kun has no adapter for {0} yet; update the kun checkout")]
    NoAdapter(String),
    #[error("the {0} seat is not open")]
    NotOpen(String),
    #[error("the app window is not available")]
    NoWindow,
    #[error("the web seats are not ready yet")]
    NoBackend,
    #[error("{0} is busy with another ask")]
    Busy(String),
    #[error("prompt is empty or too large")]
    BadPrompt,
    #[error("the page did not answer in time")]
    Timeout,
    #[error("the page went away")]
    Gone,
    #[error("{0}")]
    Page(String),
    #[error("{0}")]
    Webview(String),
}

impl Serialize for WebError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

// ----------------------------------------------------------------------------- the catalog (kun's adapters)

/// `<kun>/scripts/web/adapters`.
#[must_use]
pub fn adapters_dir(kun: &Path) -> PathBuf {
    kun.join("scripts/web/adapters")
}

/// A seat id is a short lowercase word: it names a file and a webview label, so nothing else passes.
#[must_use]
pub fn valid_seat_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 24 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Parses `index.json`: rows with a valid id and an https url; anything else is dropped.
///
/// # Errors
/// When the text is not a JSON array of seats.
pub fn parse_index(text: &str) -> Result<Vec<Seat>, WebError> {
    let rows: Vec<Seat> = serde_json::from_str(text).map_err(|_| WebError::NoIndex)?;
    Ok(rows
        .into_iter()
        .filter(|s| valid_seat_id(&s.id) && s.url.starts_with("https://") && !s.name.trim().is_empty())
        .collect())
}

/// Reads `index.json` from kun's adapters folder.
///
/// # Errors
/// [`WebError::NoIndex`] when kun has no index here or it is damaged.
pub fn load_index(dir: &Path) -> Result<Vec<Seat>, WebError> {
    let text = std::fs::read_to_string(dir.join("index.json")).map_err(|_| WebError::NoIndex)?;
    parse_index(&text)
}

fn find_seat(dir: &Path, id: &str) -> Result<Seat, WebError> {
    if !valid_seat_id(id) {
        return Err(WebError::UnknownSeat(id.to_owned()));
    }
    load_index(dir)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| WebError::UnknownSeat(id.to_owned()))
}

/// The adapter source of a seat, when kun ships one (a missing or oversized file is `None`).
fn read_adapter(dir: &Path, id: &str) -> Option<String> {
    let path = dir.join(format!("{id}.js"));
    if std::fs::metadata(&path).ok()?.len() > MAX_ADAPTER_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok().filter(|s| !s.trim().is_empty())
}

// ----------------------------------------------------------------------------- scripts (pure)

/// The initialization script: the adapter, guarded so a broken adapter never breaks the site.
#[must_use]
pub fn init_script(adapter: &str) -> String {
    format!("try {{\n{adapter}\n}} catch (e) {{ console.warn('kun web adapter', e); }}")
}

/// Wraps a body that evaluates to a JSON string so it runs once, safely, and re-injects the adapter
/// when the page lost it (`adapter` is the adapter source, or empty).
fn guarded(adapter: &str, body: &str) -> String {
    let inject = if adapter.is_empty() {
        String::new()
    } else {
        format!("if (!window.__kunWeb) {{ try {{\n{adapter}\n}} catch (e) {{}} }}\n")
    };
    format!(
        "(function () {{\n{inject}try {{\nvar w = window.__kunWeb;\nif (!w) return JSON.stringify({{ adapter: false, url: location.href }});\n{body}\n}} catch (e) {{ return JSON.stringify({{ error: String(e && e.message || e) }}); }}\n}})()"
    )
}

const STATUS_BODY: &str = "return JSON.stringify({ adapter: true, ready: !!w.ready(), needsLogin: !!w.needsLogin(), limit: w.limitHit() || null, busy: !!w.busy(), url: location.href, answer: String(w.lastAnswer() || ''), job: window.__kunJob || null });";

/// The status script: the adapter's four answers, the address, the last answer and the running send.
#[must_use]
pub fn status_script(adapter: &str) -> String {
    guarded(adapter, STATUS_BODY)
}

/// Starts `send(prompt)` in the page and returns at once; the promise's outcome lands in `window.__kunJob`.
#[must_use]
pub fn send_script(adapter: &str, prompt: &str) -> String {
    let text = serde_json::to_string(prompt).unwrap_or_else(|_| "\"\"".into());
    let body = format!(
        "window.__kunJob = {{ state: 'run' }};\nPromise.resolve().then(function () {{ return w.send({text}); }}).then(function () {{ window.__kunJob = {{ state: 'sent' }}; }}, function (e) {{ window.__kunJob = {{ state: 'fail', error: String(e && e.message || e) }}; }});\nreturn JSON.stringify({{ adapter: true, started: true }});"
    );
    guarded(adapter, &body)
}

/// The conversation so far, as `[{role, text}]`.
#[must_use]
pub fn transcript_script(adapter: &str) -> String {
    guarded(adapter, "return JSON.stringify({ adapter: true, turns: w.transcript() || [] });")
}

/// A page's evaluation result, decoded. wry hands back the JSON of the value; a script that returned
/// `JSON.stringify(x)` is therefore a JSON string of JSON, and an empty string means "no value" (an
/// exception, or `undefined`).
///
/// # Errors
/// When the page returned something that is not the JSON the scripts produce.
pub fn decode_result(raw: &str) -> Result<Value, WebError> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "null" {
        return Ok(Value::Null);
    }
    let first: Value = serde_json::from_str(raw).map_err(|e| WebError::Page(format!("unreadable page answer: {e}")))?;
    match first {
        Value::String(inner) => serde_json::from_str(&inner).map_err(|e| WebError::Page(format!("unreadable page answer: {e}"))),
        other => Ok(other),
    }
}

/// `{adapter, ready, needsLogin, limit, busy, url}` from a status script's answer.
#[must_use]
pub fn status_from(v: &Value) -> SeatStatus {
    let text = |k: &str| v[k].as_str().filter(|s| !s.trim().is_empty()).map(str::to_owned);
    SeatStatus {
        adapter: v["adapter"].as_bool().unwrap_or(false),
        ready: v["ready"].as_bool().unwrap_or(false),
        needs_login: v["needsLogin"].as_bool().unwrap_or(false),
        limit: text("limit"),
        busy: v["busy"].as_bool().unwrap_or(false),
        url: text("url"),
    }
}

// ----------------------------------------------------------------------------- the ask loop (pure core)

/// How long an ask must still wait before it may start: the gap since the last ask's start, if short.
#[must_use]
pub fn gap_wait(last_start: Option<Instant>, now: Instant, min_gap: Duration) -> Duration {
    last_start.map_or(Duration::ZERO, |t| min_gap.saturating_sub(now.saturating_duration_since(t)))
}

/// Decides when an answer is final: the seat is idle, there is a new answer (or it was seen working),
/// and the text held still for [`STABLE_FOR`]. Time is whatever clock the caller feeds.
#[derive(Debug, Clone)]
pub struct Settle {
    baseline: String,
    saw_busy: bool,
    last: String,
    still_since: Option<Duration>,
}

impl Settle {
    #[must_use]
    pub fn new(baseline: &str) -> Self {
        Self { baseline: baseline.trim().to_owned(), saw_busy: false, last: String::new(), still_since: None }
    }

    /// One observation at `at` (time since the send). `Some(answer)` when it is final.
    pub fn feed(&mut self, at: Duration, busy: bool, answer: &str) -> Option<String> {
        let answer = answer.trim();
        self.saw_busy |= busy;
        let fresh = !answer.is_empty() && (self.saw_busy || answer != self.baseline);
        if busy || !fresh {
            self.last.clear();
            self.still_since = None;
            return None;
        }
        if answer != self.last {
            answer.clone_into(&mut self.last);
            self.still_since = Some(at);
            return None;
        }
        match self.still_since {
            Some(since) if at.saturating_sub(since) >= STABLE_FOR => Some(answer.to_owned()),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------- the webview backend

/// What the ask loop needs from the app window; implemented by [`AppHandle`] so the loop does not
/// carry the runtime type (the driver reaches it through a global).
trait Backend: Send + Sync {
    fn seat_exists(&self, id: &str) -> bool;
    /// Creates the seat's webview. `visible: false` parks it off-screen and hides it.
    fn create_seat(&self, seat: &Seat, init: &str, rect: Rect, visible: bool) -> Result<(), WebError>;
    fn place_seat(&self, id: &str, rect: Rect, focus: bool) -> Result<(), WebError>;
    fn hide_seat(&self, id: &str) -> Result<(), WebError>;
    fn hide_all_seats(&self);
    fn close_seat(&self, id: &str);
    fn eval_seat(&self, id: &str, js: String, done: Box<dyn Fn(String) + Send>) -> Result<(), WebError>;
}

fn label(id: &str) -> String {
    format!("web-{id}")
}

/// The stable 16-byte id of a seat's persistent macOS data store.
#[must_use]
pub fn store_id(seat: &str) -> [u8; 16] {
    let digest = Sha256::digest(format!("org.databayt.machine/web/{seat}").as_bytes());
    let mut out = [0_u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Only https pages may load in a seat (never `tauri://`, `file:`, `data:` or a custom scheme), plus the blank start page.
#[must_use]
pub fn allowed_navigation(url: &tauri::Url) -> bool {
    url.scheme() == "https" || url.as_str() == "about:blank"
}

fn webview_err(e: impl std::fmt::Display) -> WebError {
    WebError::Webview(e.to_string())
}

fn logical(rect: Rect) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    (LogicalPosition::new(rect.x, rect.y), LogicalSize::new(rect.width.max(1.0), rect.height.max(1.0)))
}

impl<R: Runtime> Backend for AppHandle<R> {
    fn seat_exists(&self, id: &str) -> bool {
        self.get_webview(&label(id)).is_some()
    }

    fn create_seat(&self, seat: &Seat, init: &str, rect: Rect, visible: bool) -> Result<(), WebError> {
        let window = self.get_window("main").ok_or(WebError::NoWindow)?;
        let url: tauri::Url = seat.url.parse().map_err(webview_err)?;
        let name = label(&seat.id);
        let app = self.clone();
        let nav_label = name.clone();
        let mut builder = WebviewBuilder::new(&name, WebviewUrl::External(url))
            .initialization_script(init)
            .focused(visible)
            .on_navigation(allowed_navigation)
            // A sign-in popup (`window.open`) loads in the seat itself, so the flow stays in one webview.
            .on_new_window(move |url, _features| {
                if allowed_navigation(&url)
                    && let Some(w) = app.get_webview(&nav_label)
                {
                    let _ = w.navigate(url);
                }
                NewWindowResponse::Deny
            });
        if cfg!(target_os = "macos") {
            builder = builder.user_agent(SAFARI_UA).data_store_identifier(store_id(&seat.id));
        } else if let Ok(dir) = crate::pack::machine_dir() {
            builder = builder.data_directory(dir.join("web").join(&seat.id));
        }
        let start = if visible { rect } else { PARKED };
        let (pos, size) = logical(start);
        let webview = window.add_child(builder, pos, size).map_err(webview_err)?;
        if !visible {
            webview.hide().map_err(webview_err)?;
        }
        Ok(())
    }

    fn place_seat(&self, id: &str, rect: Rect, focus: bool) -> Result<(), WebError> {
        let w = self.get_webview(&label(id)).ok_or_else(|| WebError::NotOpen(id.to_owned()))?;
        let (pos, size) = logical(rect);
        w.set_bounds(tauri::Rect { position: Position::Logical(pos), size: Size::Logical(size) }).map_err(webview_err)?;
        w.show().map_err(webview_err)?;
        if focus {
            w.set_focus().map_err(webview_err)?;
        }
        Ok(())
    }

    fn hide_seat(&self, id: &str) -> Result<(), WebError> {
        match self.get_webview(&label(id)) {
            Some(w) => w.hide().map_err(webview_err),
            None => Ok(()),
        }
    }

    fn hide_all_seats(&self) {
        for (name, w) in self.webviews() {
            if name.starts_with("web-") {
                let _ = w.hide();
            }
        }
    }

    fn close_seat(&self, id: &str) {
        if let Some(w) = self.get_webview(&label(id)) {
            let _ = w.close();
        }
    }

    fn eval_seat(&self, id: &str, js: String, done: Box<dyn Fn(String) + Send>) -> Result<(), WebError> {
        let w = self.get_webview(&label(id)).ok_or_else(|| WebError::NotOpen(id.to_owned()))?;
        w.eval_with_callback(js, done).map_err(webview_err)
    }
}

// ----------------------------------------------------------------------------- shared state

#[derive(Default)]
struct Shared {
    /// The seat the Brainstorm page is showing right now.
    visible: Option<String>,
    seats: HashMap<String, SeatRun>,
}

#[derive(Default)]
struct SeatRun {
    busy: bool,
    last_start: Option<Instant>,
}

static STATE: LazyLock<Mutex<Shared>> = LazyLock::new(|| Mutex::new(Shared::default()));
static BACKEND: OnceLock<Arc<dyn Backend>> = OnceLock::new();

fn state() -> std::sync::MutexGuard<'static, Shared> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Holds a seat for one ask; dropping it (however the ask ends, or is cancelled) frees the seat.
struct SeatGuard {
    id: String,
    wait: Duration,
}

impl SeatGuard {
    /// Takes the seat, or says it is busy. `wait` is how long the ask must still hold off for the min gap.
    fn take(id: &str, now: Instant) -> Result<Self, WebError> {
        let mut s = state();
        let run = s.seats.entry(id.to_owned()).or_default();
        if run.busy {
            return Err(WebError::Busy(id.to_owned()));
        }
        run.busy = true;
        Ok(Self { id: id.to_owned(), wait: gap_wait(run.last_start, now, MIN_GAP) })
    }
}

impl Drop for SeatGuard {
    fn drop(&mut self) {
        if let Some(run) = state().seats.get_mut(&self.id) {
            run.busy = false;
        }
    }
}

fn is_visible(id: &str) -> bool {
    state().visible.as_deref() == Some(id)
}

fn backend() -> Result<Arc<dyn Backend>, WebError> {
    BACKEND.get().cloned().ok_or(WebError::NoBackend)
}

// ----------------------------------------------------------------------------- evaluation helpers

async fn eval_value(b: &dyn Backend, id: &str, js: String) -> Result<Value, WebError> {
    let (tx, rx) = oneshot::channel::<String>();
    let tx = Mutex::new(Some(tx));
    b.eval_seat(
        id,
        js,
        Box::new(move |raw| {
            if let Some(tx) = tx.lock().unwrap_or_else(PoisonError::into_inner).take() {
                let _ = tx.send(raw);
            }
        }),
    )?;
    let raw = tokio::time::timeout(EVAL_TIMEOUT, rx)
        .await
        .map_err(|_| WebError::Timeout)?
        .map_err(|_| WebError::Gone)?;
    decode_result(&raw)
}

/// The adapter source to re-inject when a page lost it; empty when kun has none.
fn adapter_text(id: &str) -> String {
    read_adapter(&adapters_dir(&crate::engine::kun_dir()), id).unwrap_or_default()
}

/// Makes sure the seat's webview exists. A new one is created parked and hidden unless `show` is given.
fn ensure(b: &dyn Backend, id: &str, show: Option<Rect>) -> Result<(), WebError> {
    if b.seat_exists(id) {
        return Ok(());
    }
    let dir = adapters_dir(&crate::engine::kun_dir());
    let seat = find_seat(&dir, id)?;
    let init = init_script(&read_adapter(&dir, id).unwrap_or_default());
    b.create_seat(&seat, &init, show.unwrap_or(PARKED), show.is_some())
}

// ----------------------------------------------------------------------------- the ask

fn ask_error(e: &WebError) -> Event {
    Event::Error { message: e.to_string() }
}

/// Waits for a freshly created or reloading seat: the adapter is in the page and `ready()`. Ends early
/// with the login / limit message when the page says so.
async fn wait_ready(b: &dyn Backend, id: &str, adapter: &str, name: &str) -> Result<(), WebError> {
    let started = Instant::now();
    let mut login_polls = 0_u32;
    while started.elapsed() < LOAD_TIMEOUT {
        if let Ok(v) = eval_value(b, id, status_script(adapter)).await {
            let st = status_from(&v);
            if let Some(limit) = st.limit {
                return Err(WebError::Page(format!("limit: {limit}")));
            }
            if st.adapter && st.ready && !st.needs_login {
                return Ok(());
            }
            login_polls = if st.adapter && st.needs_login { login_polls + 1 } else { 0 };
            if login_polls >= 3 {
                return Err(WebError::Page(format!("login: {name} needs you to sign in: open Brainstorm, {name}, and sign in once")));
            }
        }
        tokio::time::sleep(POLL_EVERY).await;
    }
    Err(WebError::Page(format!("{name} did not load: check the connection, or open Brainstorm, {name}")))
}

/// Sends `prompt` to the seat and waits for the final answer.
async fn ask_inner(b: &dyn Backend, id: &str, prompt: &str, emit: &mut (impl FnMut(Event) + Send)) -> Result<(), WebError> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        return Err(WebError::BadPrompt);
    }
    let dir = adapters_dir(&crate::engine::kun_dir());
    let seat = find_seat(&dir, id)?;
    let adapter = read_adapter(&dir, id).ok_or_else(|| WebError::NoAdapter(seat.name.clone()))?;
    let guard = SeatGuard::take(id, Instant::now())?;
    emit(Event::Tool { name: format!("{} (web)", seat.name) });
    if !guard.wait.is_zero() {
        tokio::time::sleep(guard.wait).await;
    }
    ensure(b, id, None)?;
    wait_ready(b, id, &adapter, &seat.name).await?;

    let before = eval_value(b, id, status_script(&adapter)).await?;
    let baseline = before["answer"].as_str().unwrap_or_default().to_owned();
    let reply = eval_value(b, id, send_script(&adapter, prompt)).await?;
    if reply["started"] != true {
        return Err(WebError::Page(reply["error"].as_str().unwrap_or("the page refused the prompt").to_owned()));
    }
    state().seats.entry(id.to_owned()).or_default().last_start = Some(Instant::now());

    let started = Instant::now();
    let mut settle = Settle::new(&baseline);
    while started.elapsed() < ASK_TIMEOUT {
        tokio::time::sleep(POLL_EVERY).await;
        let Ok(v) = eval_value(b, id, status_script(&adapter)).await else { continue };
        let job = &v["job"];
        if job["state"] == "fail" {
            return Err(WebError::Page(job["error"].as_str().unwrap_or("send failed").to_owned()));
        }
        let st = status_from(&v);
        if let Some(limit) = st.limit {
            return Err(WebError::Page(format!("limit: {limit}")));
        }
        if st.needs_login {
            return Err(WebError::Page(format!("login: {} signed you out: open Brainstorm, {}, and sign in", seat.name, seat.name)));
        }
        // The prompt must have left the box before an answer can count.
        if job["state"] != "sent" {
            continue;
        }
        if let Some(answer) = settle.feed(started.elapsed(), st.busy, v["answer"].as_str().unwrap_or_default()) {
            emit(Event::Text { text: answer.clone() });
            emit(Event::Done { session: None, ok: true, result: Some(answer) });
            return Ok(());
        }
    }
    Err(WebError::Page(format!("{} did not finish in {} s", seat.name, ASK_TIMEOUT.as_secs())))
}

/// Runs one ask and reports it as driver events; every failure becomes an `error` event.
async fn ask_events(b: &dyn Backend, id: &str, prompt: &str, emit: &mut (impl FnMut(Event) + Send)) {
    if let Err(e) = ask_inner(b, id, prompt, emit).await {
        emit(ask_error(&e));
    }
}

/// The driver's door: runs `prompt` on seat `id` (`web:<id>`), emitting the same events as any driver.
/// Never raises the window: the seat is hidden unless the Brainstorm page is already showing it.
pub async fn run_seat(id: &str, prompt: &str, mut emit: impl FnMut(Event) + Send) {
    match backend() {
        Ok(b) => ask_events(b.as_ref(), id, prompt, &mut emit).await,
        Err(e) => emit(ask_error(&e)),
    }
}

// ----------------------------------------------------------------------------- commands

/// `web_seats` — kun's seats and which are open here.
#[tauri::command]
async fn web_seats<R: Runtime>(app: AppHandle<R>) -> SeatList {
    let dir = adapters_dir(&crate::engine::kun_dir());
    match load_index(&dir) {
        Ok(rows) => SeatList {
            seats: rows
                .into_iter()
                .map(|s| SeatInfo { open: app.seat_exists(&s.id), adapter: read_adapter(&dir, &s.id).is_some(), id: s.id, name: s.name, url: s.url })
                .collect(),
            missing: false,
        },
        Err(_) => SeatList { seats: Vec::new(), missing: true },
    }
}

/// `web_open` — shows the seat over `rect` (creating it, signed-in state restored, on first use) and hides the others.
/// Calling it again with a new `rect` moves and resizes it.
#[tauri::command]
async fn web_open<R: Runtime>(app: AppHandle<R>, id: String, rect: Rect) -> Result<(), WebError> {
    let shown = open_seat(&app, &id, rect);
    if let Err(e) = &shown {
        tracing::warn!("web_open {id}: {e}");
    }
    shown
}

fn open_seat<R: Runtime>(app: &AppHandle<R>, id: &str, rect: Rect) -> Result<(), WebError> {
    let id = id.to_owned();
    if !valid_seat_id(&id) {
        return Err(WebError::UnknownSeat(id));
    }
    for other in app.webviews().keys().filter_map(|n| n.strip_prefix("web-")).filter(|n| *n != id) {
        let _ = app.hide_seat(other);
    }
    let focus = !is_visible(&id);
    ensure(app, &id, Some(rect))?;
    app.place_seat(&id, rect, focus)?;
    state().visible = Some(id);
    Ok(())
}

/// `web_hide` — the Brainstorm page left (or a menu is over it): every seat disappears, none is closed.
#[tauri::command]
async fn web_hide<R: Runtime>(app: AppHandle<R>) {
    state().visible = None;
    app.hide_all_seats();
}

/// `web_close` — destroys the seat's webview (the login stays on disk).
#[tauri::command]
async fn web_close<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), WebError> {
    if !valid_seat_id(&id) {
        return Err(WebError::UnknownSeat(id));
    }
    app.close_seat(&id);
    let mut s = state();
    if s.visible.as_deref() == Some(id.as_str()) {
        s.visible = None;
    }
    Ok(())
}

/// `web_status` — what the adapter says about the open seat.
#[tauri::command]
async fn web_status<R: Runtime>(app: AppHandle<R>, id: String) -> Result<SeatStatus, WebError> {
    if !valid_seat_id(&id) {
        return Err(WebError::UnknownSeat(id));
    }
    if !app.seat_exists(&id) {
        return Err(WebError::NotOpen(id));
    }
    let v = eval_value(&app, &id, status_script(&adapter_text(&id))).await?;
    let status = status_from(&v);
    tracing::debug!("web_status {id}: {status:?}");
    Ok(status)
}

/// `web_transcript` — the conversation on the page, oldest turn first.
#[tauri::command]
async fn web_transcript<R: Runtime>(app: AppHandle<R>, id: String) -> Result<Vec<Turn>, WebError> {
    if !valid_seat_id(&id) {
        return Err(WebError::UnknownSeat(id));
    }
    if !app.seat_exists(&id) {
        return Err(WebError::NotOpen(id));
    }
    let v = eval_value(&app, &id, transcript_script(&adapter_text(&id))).await?;
    if v["adapter"] == false {
        return Err(WebError::NoAdapter(id));
    }
    if let Some(e) = v["error"].as_str() {
        return Err(WebError::Page(e.to_owned()));
    }
    Ok(serde_json::from_value(v["turns"].clone()).unwrap_or_default())
}

/// `web_ask` — sends a prompt to the seat and streams the same events a driver does
/// (`tool`, then `text` and `done`, or `error`). Failures arrive as an `error` event, not a rejection.
#[tauri::command]
async fn web_ask<R: Runtime>(app: AppHandle<R>, id: String, prompt: String, channel: Channel<Event>) {
    let mut emit = |e: Event| {
        let _ = channel.send(e);
    };
    if !valid_seat_id(&id) {
        emit(ask_error(&WebError::UnknownSeat(id)));
        return;
    }
    ask_events(&app, &id, &prompt, &mut emit).await;
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("web")
        .invoke_handler(tauri::generate_handler![web_seats, web_open, web_hide, web_close, web_status, web_transcript, web_ask])
        .setup(|app, _api| {
            let _ = BACKEND.set(Arc::new(app.clone()));
            Ok(())
        })
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
