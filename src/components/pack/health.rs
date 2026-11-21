//! Department health: a pack may declare a `check` command (and a `fix` command) that prints
//! `{"ok": bool, "rows": [{"name", "level", "critical", "detail"}]}`. The result is a traffic
//! light for the home page. Commands are argv only (no shell): `node` plus one script that lives
//! under `~/.claude/skills` or kun's `.claude/skills`.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use ts_rs::TS;

/// A pack's `health` field: the check and the optional fix, each an argv.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct HealthSpec {
    #[serde(default)]
    pub check: Vec<String>,
    #[serde(default)]
    pub fix: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(rename_all = "lowercase")]
pub enum HealthLevel {
    Green,
    Orange,
    Red,
    None,
}

/// One line of a health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct HealthRow {
    pub name: String,
    /// `ok`, `warn` or `fail`.
    pub level: String,
    #[serde(default)]
    pub critical: bool,
    #[serde(default)]
    pub detail: String,
}

/// The light and what is behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct Health {
    pub level: HealthLevel,
    pub percent: u8,
    pub rows: Vec<HealthRow>,
    pub error: Option<String>,
    #[ts(type = "number")]
    pub checked_at: u64,
}

#[derive(Debug, Deserialize)]
struct Report {
    #[serde(default)]
    rows: Vec<HealthRow>,
}

pub const CHECK_TIMEOUT: Duration = Duration::from_secs(120);
pub const FIX_TIMEOUT: Duration = Duration::from_secs(900);

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// red: a critical fail; orange: any other warn or fail; green: all ok.
#[must_use]
pub fn level_of(rows: &[HealthRow]) -> HealthLevel {
    if rows.iter().any(|r| r.critical && r.level == "fail") {
        HealthLevel::Red
    } else if rows.iter().any(|r| r.level != "ok") {
        HealthLevel::Orange
    } else {
        HealthLevel::Green
    }
}

#[must_use]
pub fn percent_of(rows: &[HealthRow]) -> u8 {
    if rows.is_empty() {
        return 100;
    }
    let ok = rows.iter().filter(|r| r.level == "ok").count();
    u8::try_from(ok * 100 / rows.len()).unwrap_or(100)
}

#[must_use]
pub fn none() -> Health {
    Health { level: HealthLevel::None, percent: 0, rows: Vec::new(), error: None, checked_at: now() }
}

fn failed(error: String) -> Health {
    Health { level: HealthLevel::Red, percent: 0, rows: Vec::new(), error: Some(error), checked_at: now() }
}

/// Parses a check's stdout; the JSON may follow log noise, so a later `{` is tried too.
#[must_use]
pub fn parse_output(stdout: &str) -> Health {
    let report = serde_json::from_str::<Report>(stdout.trim()).ok().or_else(|| {
        let start = stdout.find("{\n").or_else(|| stdout.find('{'))?;
        serde_json::from_str::<Report>(stdout[start..].trim()).ok()
    });
    match report {
        Some(r) => Health {
            level: level_of(&r.rows),
            percent: percent_of(&r.rows),
            rows: r.rows,
            error: None,
            checked_at: now(),
        },
        None => failed("the check printed output that is not a health report".to_owned()),
    }
}

/// The roots a script may live under: `~/.claude/skills` and `<kun>/.claude/skills`, canonical.
fn allowed_roots(home: &Path, kun: &Path) -> Vec<PathBuf> {
    [home.join(".claude/skills"), kun.join(".claude/skills")]
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect()
}

/// Validates an argv: `node` first, then a script that resolves under an allowed root. Returns
/// the argv with `~/` expanded and the script canonical.
///
/// # Errors
/// A message when the argv is not allowed.
pub fn safe_argv(argv: &[String], home: &Path, kun: &Path) -> Result<Vec<String>, String> {
    let [prog, script, rest @ ..] = argv else {
        return Err("the health command needs `node` and a script".to_owned());
    };
    if prog != "node" {
        return Err("the health command must run `node`".to_owned());
    }
    let expanded = match script.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(script),
    };
    if !expanded.is_absolute() {
        return Err("the health script path must be absolute or start with ~/".to_owned());
    }
    let real = expanded
        .canonicalize()
        .map_err(|_| format!("health script not found: {script}"))?;
    if !real.is_file() || !allowed_roots(home, kun).iter().any(|r| real.starts_with(r)) {
        return Err("the health script is outside the skills folders".to_owned());
    }
    let mut out = vec![prog.clone(), real.display().to_string()];
    out.extend(rest.iter().cloned());
    Ok(out)
}

/// Runs one validated argv with no shell, `node` found on the GUI-safe search dirs; stdout on success.
async fn run(argv: &[String], timeout: Duration, home: &Path) -> Result<String, String> {
    let dirs = crate::driver::search_dirs();
    let node = dirs
        .iter()
        .map(|d| d.join("node"))
        .find(|p| p.is_file())
        .ok_or_else(|| "node was not found on this computer".to_owned())?;
    let mut cmd = Command::new(node);
    cmd.args(&argv[1..])
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Ok(path) = std::env::join_paths(&dirs) {
        cmd.env("PATH", path);
    }
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| format!("the check timed out after {} s", timeout.as_secs()))?
        .map_err(|e| format!("could not run the check: {e}"))?;
    // A non-zero exit still carries a report when `ok` is false, so stdout decides.
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if stdout.trim().is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no output");
        return Err(format!("the check failed: {}", line.trim()));
    }
    Ok(stdout)
}

/// Runs the pack's check (the fix first when `fix`). A pack without health gives `none`.
pub async fn evaluate(spec: Option<&HealthSpec>, home: &Path, kun: &Path, fix: bool) -> Health {
    let Some(spec) = spec.filter(|s| !s.check.is_empty()) else {
        return none();
    };
    let check = match safe_argv(&spec.check, home, kun) {
        Ok(a) => a,
        Err(e) => return failed(e),
    };
    // No fix declared: a "fix" is just a re-check, never a false red.
    if fix && !spec.fix.is_empty() {
        let fixer = match safe_argv(&spec.fix, home, kun) {
            Ok(a) => a,
            Err(e) => return failed(e),
        };
        // The fix's own report is ignored: the check re-runs and is the truth.
        if let Err(e) = run(&fixer, FIX_TIMEOUT, home).await {
            tracing::warn!("health fix: {e}");
        }
    }
    match run(&check, CHECK_TIMEOUT, home).await {
        Ok(out) => parse_output(&out),
        Err(e) => failed(e),
    }
}
