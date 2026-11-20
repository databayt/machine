//! config — the guarded write. Every change to the engine's config goes: back up the file to
//! `~/.claude/state/backups/<name>.<unix>`, validate, write atomically, run `health.sh --json`,
//! and put the backup back when the verdict goes RED. Nothing here touches a secret.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::{Map, Value};
use tokio::process::Command;

use super::model::{ConfigError, WriteOutcome};

/// Backups kept per file; older ones are deleted.
const KEEP_BACKUPS: usize = 20;

/// The health script may take a while on a slow disk.
pub(super) const HEALTH_LIMIT: Duration = Duration::from_secs(120);

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> ConfigError + '_ {
    move |e| ConfigError::Io(path.display().to_string(), e)
}

/// What the health check said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    Green,
    Warn,
    Red,
    /// The script was missing, timed out or printed nothing readable.
    Unknown,
}

impl Verdict {
    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Warn => "warn",
            Self::Red => "red",
            Self::Unknown => "unknown",
        }
    }
}

/// Reads `health.sh --json` output: the `verdict` of the last JSON object line.
pub(super) fn parse_verdict(output: &str) -> Verdict {
    output
        .lines()
        .rev()
        .filter(|l| l.trim_start().starts_with('{'))
        .find_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .and_then(|v| v.get("verdict")?.as_str().map(str::to_ascii_lowercase))
        .map_or(Verdict::Unknown, |w| match w.as_str() {
            "green" => Verdict::Green,
            "warn" => Verdict::Warn,
            "red" => Verdict::Red,
            _ => Verdict::Unknown,
        })
}

/// Runs the engine's health script and reads its verdict.
pub(super) async fn health_verdict(kun: &Path) -> Verdict {
    let script = kun.join(".claude/scripts/health.sh");
    if !script.is_file() {
        return Verdict::Unknown;
    }
    let run = Command::new("/bin/bash")
        .arg(&script)
        .arg("--json")
        .current_dir(kun)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(HEALTH_LIMIT, run).await {
        Ok(Ok(out)) => parse_verdict(&String::from_utf8_lossy(&out.stdout)),
        _ => Verdict::Unknown,
    }
}

/// A saved copy of a file as it was before a write.
#[derive(Debug, Clone)]
pub(super) struct Backup {
    pub(super) target: PathBuf,
    /// `None` when the file did not exist: restoring removes what the write created.
    pub(super) saved: Option<PathBuf>,
}

/// Copies `target` into `<claude>/state/backups/<name>.<unix>` and prunes old copies.
///
/// # Errors
/// When the backup folder or copy cannot be written.
pub(super) fn backup(
    claude: &Path,
    target: &Path,
    name: &str,
    now: u64,
) -> Result<Backup, ConfigError> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with("..") {
        return Err(ConfigError::BadName(name.to_owned()));
    }
    if !target.exists() {
        return Ok(Backup {
            target: target.to_path_buf(),
            saved: None,
        });
    }
    let dir = claude.join("state/backups");
    fs::create_dir_all(&dir).map_err(io(&dir))?;
    let saved = dir.join(format!("{name}.{now}"));
    fs::copy(target, &saved).map_err(io(&saved))?;
    prune(&dir, name);
    Ok(Backup {
        target: target.to_path_buf(),
        saved: Some(saved),
    })
}

fn prune(dir: &Path, name: &str) {
    let prefix = format!("{name}.");
    let mut copies: Vec<(u64, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let stamp = file.strip_prefix(&prefix)?.parse::<u64>().ok()?;
            Some((stamp, e.path()))
        })
        .collect();
    copies.sort();
    let excess = copies.len().saturating_sub(KEEP_BACKUPS);
    for (_, path) in copies.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

/// Writes `content` to a temp file beside `target` and renames it over, keeping the old permissions.
///
/// # Errors
/// On any filesystem failure.
pub(super) fn write_atomic(target: &Path, content: &str) -> Result<(), ConfigError> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).map_err(io(dir))?;
    let tmp = dir.join(format!(
        ".{}.machine-tmp",
        target
            .file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy())
    ));
    fs::write(&tmp, content).map_err(io(&tmp))?;
    if let Ok(meta) = fs::metadata(target) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, target).map_err(io(target))
}

impl Backup {
    /// Puts the saved content back, or removes the file the write created.
    ///
    /// # Errors
    /// On any filesystem failure.
    pub(super) fn restore(&self) -> Result<(), ConfigError> {
        match &self.saved {
            Some(saved) => {
                let text = fs::read_to_string(saved).map_err(io(saved))?;
                write_atomic(&self.target, &text)
            }
            None => match fs::remove_file(&self.target) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(io(&self.target)(e)),
            },
        }
    }

    /// Restores one top-level key of a JSON file from the backup and leaves the rest as it is now.
    /// For `~/.claude.json`, which running sessions rewrite all the time.
    ///
    /// # Errors
    /// When either file is not a JSON object or cannot be written.
    pub(super) fn restore_key(&self, key: &str) -> Result<(), ConfigError> {
        let old = match &self.saved {
            Some(saved) => read_object(saved)?,
            None => Map::new(),
        };
        let mut now = read_object(&self.target)?;
        match old.get(key) {
            Some(v) => {
                now.insert(key.to_owned(), v.clone());
            }
            None => {
                now.remove(key);
            }
        }
        write_atomic(&self.target, &pretty(&Value::Object(now)))
    }
}

/// Parses JSON text.
///
/// # Errors
/// [`ConfigError::BadJson`] with the parser's reason.
pub(super) fn validate_json(text: &str) -> Result<Value, ConfigError> {
    serde_json::from_str(text).map_err(|e| ConfigError::BadJson(e.to_string()))
}

/// Reads a JSON object file; a missing file is an empty object.
///
/// # Errors
/// When the file is not valid JSON or not an object.
pub(super) fn read_object(path: &Path) -> Result<Map<String, Value>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(e) => return Err(io(path)(e)),
    };
    match validate_json(&text)? {
        Value::Object(m) => Ok(m),
        _ => Err(ConfigError::NotObject(path.display().to_string())),
    }
}

/// Checks that text is a JSON object.
///
/// # Errors
/// When it is not valid JSON or not an object.
pub(super) fn read_object_text(text: &str) -> Result<Map<String, Value>, ConfigError> {
    match validate_json(text)? {
        Value::Object(m) => Ok(m),
        _ => Err(ConfigError::NotObject("the new settings".into())),
    }
}

pub(super) fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into());
    text.push('\n');
    text
}

/// Decides what to do with the file after the health check. A RED that was already RED before the
/// write is not the write's fault, so it is kept.
///
/// # Errors
/// When the backup has to be restored and cannot be.
pub(super) fn finish(
    backup: &Backup,
    before: Verdict,
    after: Verdict,
    restore: impl FnOnce(&Backup) -> Result<(), ConfigError>,
) -> Result<WriteOutcome, ConfigError> {
    let backup_path = backup.saved.as_ref().map(|p| p.display().to_string());
    if after == Verdict::Red && before != Verdict::Red {
        restore(backup)?;
        return Ok(WriteOutcome {
            ok: false,
            rolled_back: true,
            verdict: after.word().to_owned(),
            backup: backup_path,
            detail: "health went RED after the change, so the previous content was restored".into(),
        });
    }
    Ok(WriteOutcome {
        ok: true,
        rolled_back: false,
        verdict: after.word().to_owned(),
        backup: backup_path,
        detail: if after == Verdict::Unknown {
            "saved; health could not run".into()
        } else {
            "saved".into()
        },
    })
}
