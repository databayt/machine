//! The station: the departure board, the switch and the conductor of kun's `station.mjs`.
//!
//! Three commands run `node <kun>/.claude/scripts/station.mjs …` with no shell and return typed
//! answers: `board` (at most three home options), `fill` (one formula turned into a prompt) and
//! `match_text` (typed text to a formula id). The script is pure and fast (about 0.1 s); every
//! failure here degrades to an empty answer, never a crash and never a blocked home. The log
//! (`station-log.jsonl`) is what the script reads back to score the board.

use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{PackError, health, kun_dir, machine_dir, valid_name};

/// The script, relative to kun's checkout.
pub const SCRIPT: &str = ".claude/scripts/station.mjs";
/// The only folder a station script may live in.
const SCRIPTS_DIR: &str = ".claude/scripts";
pub const BOARD_TIMEOUT: Duration = Duration::from_secs(5);
pub const MATCH_TIMEOUT: Duration = Duration::from_secs(5);
pub const FILL_TIMEOUT: Duration = Duration::from_secs(10);
/// The home shows at most this many options.
pub const MAX_OPTIONS: usize = 3;
/// The longest typed text sent to the switch, in characters.
const MAX_TEXT_CHARS: usize = 500;
/// The longest slot value sent to `fill`, in characters.
const MAX_SLOT_CHARS: usize = 300;

/// A map of facets whose values may arrive as strings, numbers or booleans.
fn lenient_map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    let raw = Option::<BTreeMap<String, Value>>::deserialize(d)?.unwrap_or_default();
    Ok(raw
        .into_iter()
        .filter_map(|(k, v)| {
            let v = match v {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => return None,
            };
            Some((k, v))
        })
        .collect())
}

/// One of the options on the home, already ordered by the script.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct BoardOption {
    /// `waiting`, `next` or `start`.
    pub role: String,
    /// The formula id, e.g. `video.film.plan`.
    pub id: String,
    pub label_en: String,
    pub label_ar: String,
    /// A lucide icon name; the UI falls back to a bulb.
    pub icon: Option<String>,
    /// The route id the formula runs in.
    pub route: Option<String>,
    /// `issue` or `none`: whether accepting the plan records an issue.
    pub record: String,
    pub score: f64,
    pub why: Vec<String>,
    /// The slot values to hand back to `fill`.
    #[serde(deserialize_with = "lenient_map")]
    pub set: BTreeMap<String, String>,
}

/// The departure board: zero to three options. `error` says why it is empty when it failed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct Board {
    pub department: String,
    pub repo: Option<String>,
    pub options: Vec<BoardOption>,
    pub error: Option<String>,
}

/// A local reference the formula wants on the CDN: where it is, where it goes and its link.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct FillUpload {
    pub local: String,
    pub key: String,
    pub url: String,
}

/// One formula, filled. Unfilled slots stay in `prompt` as the script writes them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct Filled {
    pub id: String,
    pub prompt: String,
    pub route: Option<String>,
    pub record: String,
    pub missing: Vec<String>,
    /// The formula the request asked for when the script moved it to another.
    pub redirected_from: Option<String>,
    pub uploads: Vec<FillUpload>,
}

/// The switch's answer for typed text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct Match {
    /// The formula id, or none.
    pub id: Option<String>,
    pub confidence: f64,
    /// Another department when the text names one.
    pub dept: Option<String>,
}

/// A formula id: `video.film.plan`.
#[must_use]
pub fn valid_formula_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with(['.', '-'])
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
        && !s.contains("..")
}

fn valid_slot_key(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn valid_slot_value(s: &str) -> bool {
    s.chars().count() <= MAX_SLOT_CHARS && !s.chars().any(char::is_control)
}

fn valid_repo(s: &str) -> bool {
    valid_name(s)
}

/// The leading arguments every call shares.
fn common(dept: &str, repo: Option<&str>, login: &str) -> Result<Vec<String>, PackError> {
    if !valid_name(dept) {
        return Err(PackError::BadName);
    }
    let mut a = vec!["--dept".to_owned(), dept.to_owned()];
    if let Some(r) = repo.filter(|r| !r.is_empty()) {
        if !valid_repo(r) {
            return Err(PackError::BadRepo(r.to_owned()));
        }
        a.extend(["--repo".to_owned(), r.to_owned()]);
    }
    if !login.is_empty() && crate::profile::valid_login(login) {
        a.extend(["--login".to_owned(), login.to_owned()]);
    }
    Ok(a)
}

/// The arguments of `board`, after the script path.
///
/// # Errors
/// On a bad department or repo name.
pub fn board_args(
    dept: &str,
    repo: Option<&str>,
    login: &str,
    health: Option<&str>,
) -> Result<Vec<String>, PackError> {
    let mut a = vec!["board".to_owned()];
    a.extend(common(dept, repo, login)?);
    if let Some(h) = health.filter(|h| matches!(*h, "green" | "orange" | "red")) {
        a.extend(["--health".to_owned(), h.to_owned()]);
    }
    a.push("--json".to_owned());
    Ok(a)
}

/// The arguments of `fill`, after the script path.
///
/// # Errors
/// On a bad formula id, department, repo or slot.
pub fn fill_args(
    dept: &str,
    id: &str,
    repo: Option<&str>,
    login: &str,
    set: &BTreeMap<String, String>,
) -> Result<Vec<String>, PackError> {
    if !valid_formula_id(id) {
        return Err(PackError::Station("formula id is invalid".to_owned()));
    }
    let mut a = vec!["fill".to_owned(), id.to_owned()];
    a.extend(common(dept, repo, login)?);
    for (k, v) in set {
        if !valid_slot_key(k) || !valid_slot_value(v) {
            return Err(PackError::Station(format!("slot `{k}` is invalid")));
        }
        a.extend(["--set".to_owned(), format!("{k}={v}")]);
    }
    a.push("--json".to_owned());
    Ok(a)
}

/// The text the switch reads: trimmed, at most [`MAX_TEXT_CHARS`], and without leading dashes so it
/// can never read as a flag. `None` when nothing is left.
#[must_use]
pub fn clean_text(text: &str) -> Option<String> {
    let t: String = text
        .trim()
        .trim_start_matches('-')
        .trim_start()
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .take(MAX_TEXT_CHARS)
        .collect();
    let t = t.trim().to_owned();
    (!t.is_empty()).then_some(t)
}

/// The arguments of `match`, after the script path.
///
/// # Errors
/// On a bad department or repo name; `Ok(None)` when the text is empty.
pub fn match_args(dept: &str, text: &str, repo: Option<&str>) -> Result<Option<Vec<String>>, PackError> {
    let Some(text) = clean_text(text) else {
        return Ok(None);
    };
    let mut a = vec!["match".to_owned(), text];
    a.extend(common(dept, repo, "")?);
    a.push("--json".to_owned());
    Ok(Some(a))
}

/// The JSON object in a script's stdout; it may follow log noise, so a later `{` is tried too.
fn json_of(stdout: &str) -> Option<Value> {
    serde_json::from_str::<Value>(stdout.trim()).ok().or_else(|| {
        let start = stdout.find("{\n").or_else(|| stdout.find('{'))?;
        serde_json::from_str::<Value>(stdout[start..].trim()).ok()
    })
}

/// The board in the script's stdout. Options are read one by one (a bad one is skipped), the first
/// [`MAX_OPTIONS`] with an id are kept. Anything that is not a board is an empty one with its reason.
#[must_use]
pub fn parse_board(stdout: &str, dept: &str, repo: Option<&str>) -> Board {
    let empty = |error: Option<String>| Board {
        department: dept.to_owned(),
        repo: repo.filter(|r| !r.is_empty()).map(str::to_owned),
        options: Vec::new(),
        error,
    };
    let Some(v) = json_of(stdout) else {
        return empty(Some("the station printed something that is not a board".to_owned()));
    };
    let options: Vec<BoardOption> = v
        .get("options")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| {
            a.iter()
                .filter_map(|o| serde_json::from_value::<BoardOption>(o.clone()).ok())
                .filter(|o| valid_formula_id(&o.id))
                .take(MAX_OPTIONS)
                .collect()
        });
    let error = v.get("error").and_then(Value::as_str).map(str::to_owned);
    Board { options, error, ..empty(None) }
}

/// A filled formula from the script's stdout.
///
/// # Errors
/// When the output is not a fill, or the script reported an error.
pub fn parse_fill(stdout: &str) -> Result<Filled, PackError> {
    let v = json_of(stdout).ok_or_else(|| PackError::Station("the station printed something that is not a prompt".to_owned()))?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return Err(PackError::Station(e.to_owned()));
    }
    let f: Filled = serde_json::from_value(v)
        .map_err(|e| PackError::Station(format!("the station's prompt was not understood: {e}")))?;
    if f.id.is_empty() {
        return Err(PackError::Station("the station returned no formula".to_owned()));
    }
    Ok(f)
}

/// The switch's answer; anything else is "no match".
#[must_use]
pub fn parse_match(stdout: &str) -> Match {
    let Some(v) = json_of(stdout) else {
        return Match::default();
    };
    let mut m: Match = serde_json::from_value(v).unwrap_or_default();
    m.id = m.id.filter(|i| valid_formula_id(i));
    m.dept = m.dept.filter(|d| valid_name(d));
    if !m.confidence.is_finite() {
        m.confidence = 0.0;
    }
    m
}

/// The login saved in settings; empty when none.
fn saved_login() -> String {
    crate::settings::load().map(|s| s.login).unwrap_or_default()
}

/// Runs the station script with `args`. The script must resolve under `<kun>/.claude/scripts`.
async fn run_station(args: Vec<String>, timeout: Duration) -> Result<String, String> {
    let kun = kun_dir().map_err(|e| e.to_string())?;
    let home = dirs::home_dir().ok_or("could not find the home folder")?;
    run_station_in(&kun, &home, args, timeout).await
}

async fn run_station_in(kun: &Path, home: &Path, args: Vec<String>, timeout: Duration) -> Result<String, String> {
    let mut cmd = vec!["node".to_owned(), kun.join(SCRIPT).display().to_string()];
    cmd.extend(args);
    let checked = health::safe_argv_under(&cmd, home, &[kun.join(SCRIPTS_DIR)])?;
    health::run(&checked, timeout, home).await
}

/// The board for a department. Never fails: a failure is an empty board that says why.
pub async fn board(dept: &str, repo: Option<&str>, health_level: Option<&str>) -> Board {
    let login = saved_login();
    let empty = |why: String| Board {
        department: dept.to_owned(),
        repo: repo.filter(|r| !r.is_empty()).map(str::to_owned),
        options: Vec::new(),
        error: Some(why),
    };
    let args = match board_args(dept, repo, &login, health_level) {
        Ok(a) => a,
        Err(e) => return empty(e.to_string()),
    };
    match run_station(args, BOARD_TIMEOUT).await {
        Ok(out) => parse_board(&out, dept, repo),
        Err(e) => empty(e),
    }
}

/// One formula filled with `set`.
///
/// # Errors
/// A message when the arguments are bad, the script is not available or its answer is not a prompt.
pub async fn fill(
    dept: &str,
    id: &str,
    repo: Option<&str>,
    set: &BTreeMap<String, String>,
) -> Result<Filled, PackError> {
    let args = fill_args(dept, id, repo, &saved_login(), set)?;
    let out = run_station(args, FILL_TIMEOUT).await.map_err(PackError::Station)?;
    parse_fill(&out)
}

/// Typed text to a formula. Never fails: a failure is "no match".
pub async fn match_text(dept: &str, text: &str, repo: Option<&str>) -> Match {
    let Ok(Some(args)) = match_args(dept, text, repo) else {
        return Match::default();
    };
    match run_station(args, MATCH_TIMEOUT).await {
        Ok(out) => parse_match(&out),
        Err(e) => {
            tracing::warn!("station match: {e}");
            Match::default()
        }
    }
}

/// What the person did with an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(rename_all = "lowercase")]
pub enum LogEvent {
    Pick,
    Dismiss,
}

/// One line of the log: `{ts, login, dept, repo, id, event}`.
#[derive(Serialize)]
struct LogLine<'a> {
    ts: &'a str,
    login: &'a str,
    dept: &'a str,
    repo: Option<&'a str>,
    id: &'a str,
    event: LogEvent,
}

/// The log line for one event, without the newline.
#[must_use]
pub fn log_line(ts: &str, login: &str, dept: &str, repo: Option<&str>, id: &str, event: LogEvent) -> String {
    let repo = repo.filter(|r| !r.is_empty());
    serde_json::to_string(&LogLine { ts, login, dept, repo, id, event }).unwrap_or_default()
}

/// A UTC time as `2026-11-22T09:30:00Z`, from seconds since the epoch.
#[must_use]
pub fn iso_utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// `<machine>/station-log.jsonl`, the file the script reads.
#[must_use]
pub fn log_path(machine: &Path) -> PathBuf {
    machine.join("station-log.jsonl")
}

/// Appends one line (and a newline) in a single write.
fn append_line(path: &Path, line: &str) -> Result<(), PackError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| PackError::Io(dir.display().to_string(), e))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| PackError::Io(path.display().to_string(), e))?;
    file.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| PackError::Io(path.display().to_string(), e))
}

/// Records a tap or a dismissal in the machine folder's log.
///
/// # Errors
/// On a bad department, repo or id, or a filesystem failure.
pub fn log_event(dept: &str, repo: Option<&str>, id: &str, event: LogEvent) -> Result<(), PackError> {
    log_event_in(&machine_dir()?, &saved_login(), dept, repo, id, event, now_secs())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub(super) fn log_event_in(
    machine: &Path,
    login: &str,
    dept: &str,
    repo: Option<&str>,
    id: &str,
    event: LogEvent,
    now: u64,
) -> Result<(), PackError> {
    if !valid_name(dept) {
        return Err(PackError::BadName);
    }
    if let Some(r) = repo.filter(|r| !r.is_empty())
        && !valid_repo(r)
    {
        return Err(PackError::BadRepo(r.to_owned()));
    }
    if !valid_formula_id(id) {
        return Err(PackError::Station("formula id is invalid".to_owned()));
    }
    let login = if crate::profile::valid_login(login) { login } else { "" };
    append_line(&log_path(machine), &log_line(&iso_utc(now), login, dept, repo, id, event))
}

#[cfg(test)]
#[path = "station_test.rs"]
mod tests;
