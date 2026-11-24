//! driver — runs a prompt through the agent CLI the teammate is already signed into.
//!
//! machine never proxies a model and never touches a login: it spawns the unmodified
//! `claude`, `codex`, `opencode`, `agy` or `hermes` binary on the owner's own machine and reads its JSON
//! event stream. The parsing is pure (`parse_line`) so it is tested against fixtures.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use ts_rs::TS;

/// Which agent CLI runs the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "driver/types.ts")]
pub enum DriverKind {
    Claude,
    Codex,
    Opencode,
    /// Google Antigravity CLI.
    Agy,
    /// Hermes agent, one-shot mode only.
    Hermes,
    /// kun's free lane (`f`, `kun-free.sh`): Claude Code's CLI on a free model, same stream-json.
    Free,
}

impl DriverKind {
    pub const ALL: [Self; 6] = [
        Self::Claude,
        Self::Codex,
        Self::Opencode,
        Self::Agy,
        Self::Hermes,
        Self::Free,
    ];

    #[must_use]
    pub fn binary(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Opencode => "opencode",
            Self::Agy => "agy",
            Self::Hermes => "hermes",
            Self::Free => "kun-free.sh",
        }
    }

    /// Other names the same program is installed under (the `f` launcher for `kun-free.sh`).
    #[must_use]
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Free => &["f"],
            _ => &[],
        }
    }

    /// Whether the job speaks Claude Code's flags and stream-json events.
    #[must_use]
    pub fn claude_like(self) -> bool {
        matches!(self, Self::Claude | Self::Free)
    }
}

/// The program for `kind` in `dirs`, under its own name first, then its aliases.
pub(crate) fn find_driver(kind: DriverKind, dirs: &[PathBuf]) -> Option<PathBuf> {
    std::iter::once(kind.binary())
        .chain(kind.aliases().iter().copied())
        .find_map(|name| find_binary(name, dirs))
}

/// Whether a driver is usable on this machine.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "driver/types.ts")]
pub struct DriverStatus {
    pub kind: DriverKind,
    pub installed: bool,
    pub path: Option<String>,
}

/// How much Claude may do on its own in a run. `Plan` makes the first answer a plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "driver/types.ts")]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    #[default]
    Auto,
    Plan,
}

impl PermissionMode {
    /// The allow-list for what the UI sends: `"plan"` and `"auto"`; anything else is auto.
    #[must_use]
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("plan") => Self::Plan,
            _ => Self::Auto,
        }
    }

    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Plan => "plan",
        }
    }
}

/// One unit of work: a prompt, where to run it, and which session to resume.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "driver/types.ts")]
pub struct Job {
    pub job_id: String,
    pub driver: DriverKind,
    pub prompt: String,
    pub cwd: String,
    pub session: Option<String>,
    pub timeout_secs: Option<u32>,
    /// Extra flags, already paired as `[flag, value, flag, value…]`. Only an allow-listed few
    /// are accepted (`--add-dir`, `--settings`, claude only); see [`args_for`].
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Environment for this one run (a profile's secrets). Never sent to or from JS.
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// Claude's `--permission-mode`; the other drivers ignore it.
    #[serde(default)]
    pub permission_mode: PermissionMode,
}

/// What the UI sees while a job runs. Normalized across drivers.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[ts(export, export_to = "driver/types.ts")]
pub enum Event {
    Started {
        session: String,
    },
    Text {
        text: String,
    },
    Tool {
        name: String,
    },
    Done {
        session: Option<String>,
        ok: bool,
        result: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("{0} is not installed or not on PATH")]
    NotInstalled(&'static str),
    #[error("job is invalid: {0}")]
    Invalid(&'static str),
    #[error("could not start {0}: {1}")]
    Spawn(&'static str, std::io::Error),
    #[error("timed out after {0}s")]
    Timeout(u32),
}

impl Serialize for DriverError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

const DEFAULT_TIMEOUT_SECS: u32 = 30 * 60;
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_EXTRA_ARGS: usize = 32;
/// The only flags a caller may add, each with one value.
const EXTRA_FLAGS: [&str; 5] = ["--add-dir", "--settings", "--model", "--effort", "--agent"];
/// Environment names a run may never override.
const PROTECTED_ENV: [&str; 6] = [
    "PATH",
    "HOME",
    "USERPROFILE",
    "SHELL",
    "NODE_OPTIONS",
    "CLAUDE_CONFIG_DIR",
];

/// Apps launched from the Finder or Start menu get a minimal PATH, so look where
/// the CLIs' own installers put them before falling back to PATH.
pub(crate) fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        for sub in [
            ".local/bin",
            ".opencode/bin",
            ".npm-global/bin",
            ".bun/bin",
            ".cargo/bin",
        ] {
            dirs.push(home.join(sub));
        }
    }
    if cfg!(windows) {
        dirs.extend(windows_dirs(
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            std::env::var_os("USERPROFILE").map(PathBuf::from),
            std::env::var_os("APPDATA").map(PathBuf::from),
        ));
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

/// `dirs` as a PATH value: existing folders only, each once, in order.
pub(crate) fn run_path(dirs: &[PathBuf]) -> std::ffi::OsString {
    let mut seen: Vec<&PathBuf> = Vec::new();
    for d in dirs {
        if d.is_dir() && !seen.contains(&d) {
            seen.push(d);
        }
    }
    std::env::join_paths(seen).unwrap_or_default()
}

/// Where the Windows installers put the CLIs: `%LOCALAPPDATA%\Programs\claude`,
/// `%USERPROFILE%\.local\bin` and the npm global `%APPDATA%\npm`.
fn windows_dirs(
    local_app_data: Option<PathBuf>,
    user_profile: Option<PathBuf>,
    app_data: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(p) = local_app_data {
        dirs.push(p.join("Programs").join("claude"));
    }
    if let Some(p) = user_profile {
        dirs.push(p.join(".local").join("bin"));
    }
    if let Some(p) = app_data {
        dirs.push(p.join("npm"));
    }
    dirs
}

pub(crate) fn find_binary(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd"]
    } else {
        &[""]
    };
    dirs.iter().find_map(|dir| {
        candidates
            .iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

/// Which drivers are installed here. Being signed in is checked by running them.
#[must_use]
pub fn detect() -> Vec<DriverStatus> {
    let dirs = search_dirs();
    DriverKind::ALL
        .into_iter()
        .map(|kind| {
            let path = find_driver(kind, &dirs);
            DriverStatus {
                kind,
                installed: path.is_some(),
                path: path.map(|p| p.display().to_string()),
            }
        })
        .collect()
}

/// Extra flags are `--add-dir <dir>` / `--settings <file>` pairs and only for claude; the
/// environment may not touch the variables that decide which binary or config runs.
fn validate_extras(job: &Job) -> Result<(), DriverError> {
    // Fable bills usage credits with no consent prompt and never runs unattended (kun rule),
    // so a model string naming it is refused for every driver, wherever it hides.
    let fable = |a: &String| a.to_ascii_lowercase().contains("fable");
    if job.extra_args.iter().any(fable) || job.env.iter().any(|(k, v)| fable(k) || fable(v)) {
        return Err(DriverError::Invalid("model"));
    }
    if !job.extra_args.is_empty() {
        let tame = |a: &String| a.len() <= 4096 && !a.chars().any(char::is_control);
        if !job.driver.claude_like() {
            return Err(DriverError::Invalid("extra args are claude-only"));
        }
        if job.extra_args.len() > MAX_EXTRA_ARGS
            || !job.extra_args.len().is_multiple_of(2)
            || !job.extra_args.iter().all(tame)
        {
            return Err(DriverError::Invalid("extra args"));
        }
        for pair in job.extra_args.chunks_exact(2) {
            if !EXTRA_FLAGS.contains(&pair[0].as_str())
                || pair[1].is_empty()
                || pair[1].starts_with('-')
            {
                return Err(DriverError::Invalid("extra args"));
            }
        }
    }
    for (name, value) in &job.env {
        let name_ok = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            && !PROTECTED_ENV.contains(&name.as_str())
            && !name.starts_with("LD_")
            && !name.starts_with("DYLD_");
        if !name_ok || value.len() > 8192 || value.contains('\0') {
            return Err(DriverError::Invalid("env"));
        }
    }
    Ok(())
}

/// The exact argv for a job. Pure, so it is unit-tested.
///
/// # Errors
/// When the prompt is empty or too large, or the session id has unsafe characters.
pub fn args_for(job: &Job) -> Result<Vec<String>, DriverError> {
    if job.prompt.trim().is_empty() {
        return Err(DriverError::Invalid("empty prompt"));
    }
    if job.prompt.len() > MAX_PROMPT_BYTES {
        return Err(DriverError::Invalid("prompt too large"));
    }
    if let Some(s) = &job.session
        && !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(DriverError::Invalid("session id"));
    }
    validate_extras(job)?;
    // agy and hermes take the prompt as the value of their own flag, so they return early
    // (a value that starts with "-" is still read as the value, verified on both CLIs).
    match job.driver {
        // agy: `-p <prompt>` is print mode; stream-json is its NDJSON event stream.
        DriverKind::Agy => {
            let mut a: Vec<String> = ["-p", job.prompt.as_str(), "--output-format", "stream-json", "--mode", "auto"]
                .map(String::from)
                .to_vec();
            if let Some(s) = &job.session {
                a.extend(["--conversation".to_string(), s.clone()]);
            }
            return Ok(a);
        }
        // hermes: `--oneshot=<prompt>` prints only the final text. It is attached with `=` so
        // a prompt starting with "-" is not read as a flag. Hermes only ever starts from the
        // owner's own click, same policy as codex: oneshot auto-bypasses approvals.
        DriverKind::Hermes => {
            let mut a = Vec::new();
            if let Some(s) = &job.session {
                a.extend(["--resume".to_string(), s.clone()]);
            }
            a.push(format!("--oneshot={}", job.prompt));
            return Ok(a);
        }
        _ => {}
    }
    let mut args: Vec<String> = match job.driver {
        DriverKind::Claude | DriverKind::Free => {
            let mut a: Vec<String> = [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--permission-mode",
                job.permission_mode.flag(),
            ]
            .into_iter()
            .map(String::from)
            .collect();
            a.extend(job.extra_args.iter().cloned());
            if job.session.is_some() {
                a.push("--resume".into());
            }
            a
        }
        // Codex: workspace-write sandbox; docs discourage unattended ChatGPT-plan runs,
        // so machine only ever starts codex from the owner's own click.
        DriverKind::Codex => ["exec", "--json", "--sandbox", "workspace-write"]
            .map(String::from)
            .to_vec(),
        DriverKind::Opencode => {
            let mut a = vec!["run".to_string(), "--format".into(), "json".into()];
            if job.session.is_some() {
                a.push("--session".into());
            }
            a
        }
        DriverKind::Agy | DriverKind::Hermes => unreachable!("returned above"),
    };
    if let Some(s) = &job.session {
        match job.driver {
            DriverKind::Codex => args.extend(["resume".to_string(), s.clone()]),
            _ => args.push(s.clone()),
        }
    }
    // `--` ends option parsing, so a prompt that starts with "-" is never read as a flag.
    args.push("--".into());
    args.push(job.prompt.clone());
    Ok(args)
}

/// Turns one line of a driver's JSON stream into zero or more events.
#[must_use]
pub fn parse_line(kind: DriverKind, line: &str) -> Vec<Event> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let s = |v: &Value| v.as_str().map(str::to_owned);
    match kind {
        DriverKind::Claude | DriverKind::Free => match v["type"].as_str() {
            Some("system") if v["subtype"] == "init" => s(&v["session_id"])
                .map(|session| Event::Started { session })
                .into_iter()
                .collect(),
            Some("assistant") => v["message"]["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|part| match part["type"].as_str() {
                    Some("text") => s(&part["text"]).map(|text| Event::Text { text }),
                    Some("tool_use") => s(&part["name"]).map(|name| Event::Tool { name }),
                    _ => None,
                })
                .collect(),
            Some("result") => vec![Event::Done {
                session: s(&v["session_id"]),
                ok: v["is_error"] == false,
                result: s(&v["result"]),
            }],
            _ => Vec::new(),
        },
        DriverKind::Codex => match v["type"].as_str() {
            Some("thread.started") => s(&v["thread_id"])
                .map(|session| Event::Started { session })
                .into_iter()
                .collect(),
            Some("item.completed") => match v["item"]["type"].as_str() {
                Some("agent_message") => s(&v["item"]["text"])
                    .map(|text| Event::Text { text })
                    .into_iter()
                    .collect(),
                Some("command_execution") => vec![Event::Tool {
                    name: "shell".into(),
                }],
                Some(other) => vec![Event::Tool {
                    name: other.to_owned(),
                }],
                None => Vec::new(),
            },
            Some("turn.completed") => vec![Event::Done {
                session: None,
                ok: true,
                result: None,
            }],
            Some("error") => s(&v["message"])
                .map(|message| Event::Error { message })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        },
        // agy stream-json: `{"event":"result","result":{conversation_id,status,response,error,..}}`.
        // Only the result line is verified against a real run (the account was ineligible, so
        // the captured fixture is the ERROR form); other event names are ignored until seen.
        DriverKind::Agy => {
            if v["event"] != "result" {
                return Vec::new();
            }
            let r = &v["result"];
            let failed = r["status"] == "ERROR" || r["error"].as_str().is_some_and(|e| !e.is_empty());
            if failed {
                let message = s(&r["error"]).unwrap_or_else(|| "agy failed".into());
                return vec![Event::Error { message }];
            }
            let session = s(&r["conversation_id"]).filter(|c| !c.is_empty());
            let mut out = Vec::new();
            if let Some(text) = s(&r["response"]).filter(|t| !t.is_empty()) {
                out.push(Event::Text { text: text.clone() });
            }
            out.push(Event::Done { session, ok: true, result: s(&r["response"]) });
            out
        }
        // hermes prints plain text, not JSON: `hermes_events` handles the whole stdout.
        DriverKind::Hermes => Vec::new(),
        DriverKind::Opencode => match v["type"].as_str() {
            Some("error") => {
                let message = s(&v["error"]["data"]["message"])
                    .or_else(|| s(&v["error"]["name"]))
                    .unwrap_or_default();
                vec![Event::Error { message }]
            }
            Some("text") => s(&v["part"]["text"])
                .map(|text| Event::Text { text })
                .into_iter()
                .collect(),
            Some("tool_use") => s(&v["part"]["tool"])
                .map(|name| Event::Tool { name })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        },
    }
}

/// Hermes one-shot output is the final response as plain text with no events, so the whole
/// stdout becomes one text event followed by done.
#[must_use]
pub fn hermes_events(stdout: &str) -> Vec<Event> {
    let text = stdout.trim();
    let mut out = Vec::new();
    if !text.is_empty() {
        out.push(Event::Text { text: text.to_owned() });
    }
    out.push(Event::Done { session: None, ok: !text.is_empty(), result: (!text.is_empty()).then(|| text.to_owned()) });
    out
}

/// A run that points claude at a proxy (`ANTHROPIC_BASE_URL` in its env) must not inherit another
/// `ANTHROPIC_*` variable or the telemetry switch from the app's own environment: a stray
/// `ANTHROPIC_API_KEY` would be sent to the proxy, and `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`
/// breaks it. Returns the inherited names to drop; a normal run drops nothing.
fn inherited_to_remove(job: &Job, inherited: impl Iterator<Item = String>) -> Vec<String> {
    if !job.driver.claude_like() || !job.env.iter().any(|(k, _)| k == "ANTHROPIC_BASE_URL") {
        return Vec::new();
    }
    inherited
        .filter(|k| k.starts_with("ANTHROPIC_") || k == "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC")
        .filter(|k| !job.env.iter().any(|(n, _)| n == k))
        .collect()
}

/// Runs a job to completion, calling `emit` for every event. Never panics on bad output.
///
/// # Errors
/// When the driver is missing, the job is invalid, the process cannot start, or it times out.
pub async fn run(job: &Job, mut emit: impl FnMut(Event) + Send) -> Result<(), DriverError> {
    let bin = find_driver(job.driver, &search_dirs())
        .ok_or(DriverError::NotInstalled(job.driver.binary()))?;
    let args = args_for(job)?;
    if !Path::new(&job.cwd).is_dir() {
        return Err(DriverError::Invalid("cwd is not a folder"));
    }
    let mut cmd = Command::new(bin);
    for key in inherited_to_remove(job, std::env::vars_os().filter_map(|(k, _)| k.into_string().ok())) {
        cmd.env_remove(key);
    }
    // A Finder launch hands the app a bare PATH (`/usr/bin:/bin:…`), so the run's own tools (node,
    // ffmpeg, blender under /opt/homebrew/bin) were missing. Give it the same dirs the CLI was found in.
    cmd.env("PATH", run_path(&search_dirs()));
    let mut child = cmd
        .args(&args)
        .current_dir(&job.cwd)
        .envs(job.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| DriverError::Spawn(job.driver.binary(), e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or(DriverError::Invalid("no stdout"))?;
    let mut lines = BufReader::new(stdout).lines();
    let secs = job.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
    let mut finished = false;
    let read = async {
        let mut plain = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if job.driver == DriverKind::Hermes {
                plain.push_str(&line);
                plain.push('\n');
                continue;
            }
            for event in parse_line(job.driver, &line) {
                finished |= matches!(event, Event::Done { .. } | Event::Error { .. });
                emit(event);
            }
        }
        let status = child.wait().await;
        if job.driver == DriverKind::Hermes {
            let ok = status.is_ok_and(|s| s.success());
            for event in hermes_events(&plain) {
                emit(match event {
                    Event::Done { session, result, .. } => Event::Done { session, ok: ok && result.is_some(), result },
                    e => e,
                });
            }
        } else if !finished {
            let ok = status.is_ok_and(|s| s.success());
            emit(Event::Done {
                session: None,
                ok,
                result: None,
            });
        }
    };
    tokio::time::timeout(Duration::from_secs(u64::from(secs)), read)
        .await
        .map_err(|_| DriverError::Timeout(secs))
}

/// Why a Claude run could not start at all: the person's own Claude is unavailable. Only these
/// cases let the chat fall back to a free route; every other failure stays an ordinary error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// No sign-in, an expired or revoked one, or an invalid key.
    NotSignedIn,
    /// The plan's usage (or spend) limit is reached; `resets_at` is the CLI's own wording, e.g. `9pm (Asia/Riyadh)`.
    UsageLimit { resets_at: Option<String> },
    /// The account has no usable subscription or billing (organization disabled, credit balance too low).
    NoSubscription,
}

/// Longest message that can be one of the CLI's own availability lines (a long body is a real answer).
const UNAVAILABLE_MAX_CHARS: usize = 400;

/// Classifies the text of a failed Claude run. The wordings are the Claude Code CLI's own
/// (2.1.330 bundle): `You've hit your limit · resets 9pm (Asia/Riyadh)`, `usage limit reached`,
/// `Invalid API key · Please run /login`, `Not logged in`, `Session expired. Please run /login`,
/// `Credit balance is too low`, `organization has been disabled`. Anything else is `None`.
#[must_use]
pub fn classify_unavailable(text: &str) -> Option<Unavailable> {
    if text.chars().count() > UNAVAILABLE_MAX_CHARS {
        return None;
    }
    let low = text.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| low.contains(n));
    if has(&[
        "you've hit your",
        "you've reached your",
        "usage limit reached",
        "out of usage credits",
        "out of extra usage",
        "spend limit reached",
    ]) {
        let resets_at = low.find("resets ").map(|i| {
            let rest = &text[i + "resets ".len()..];
            let end = rest.find(['·', '\n', '|']).unwrap_or(rest.len());
            rest[..end].trim().to_owned()
        });
        return Some(Unavailable::UsageLimit { resets_at: resets_at.filter(|r| !r.is_empty()) });
    }
    if has(&["credit balance is too low", "credit balance too low", "organization has been disabled", "requires a claude.ai subscription"]) {
        return Some(Unavailable::NoSubscription);
    }
    if has(&[
        "please run /login",
        "not logged in",
        "invalid api key",
        "oauth token revoked",
        "login expired",
        "run claude auth login",
    ]) {
        return Some(Unavailable::NotSignedIn);
    }
    None
}

/// Whether a finished Claude run failed because Claude is unavailable: its closing `Done { ok: false }`
/// result or its `Error` message classifies. A successful run, or any other failure, is `None`.
#[must_use]
pub fn classify_failure(events: &[Event]) -> Option<Unavailable> {
    match events.iter().rev().find(|e| matches!(e, Event::Done { .. } | Event::Error { .. }))? {
        Event::Done { ok: false, result: Some(r), .. } => classify_unavailable(r),
        Event::Error { message } => classify_unavailable(message),
        _ => None,
    }
}

/// `drivers` — which agent CLIs this machine has.
#[tauri::command]
fn drivers() -> Vec<DriverStatus> {
    detect()
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("driver")
        .invoke_handler(tauri::generate_handler![drivers])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
