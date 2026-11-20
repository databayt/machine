//! config — which keys of `~/.claude/settings.json` this app may edit: `model`, `voice`, and every
//! key kun's own template (`<kun>/.claude/settings.json`) does not manage. Keys that run commands or
//! grant permissions are never editable here, whatever the template says.

use std::{collections::BTreeSet, fs, path::Path};

use serde_json::{Map, Value};

use super::{
    guard,
    model::{ConfigError, SettingEntry},
};

/// Always editable: personal choices kun does not manage.
const ALWAYS: [&str; 2] = ["model", "voice"];

/// Never editable here: they run commands, grant permissions or carry credentials.
const NEVER: [&str; 11] = [
    "hooks",
    "env",
    "permissions",
    "apiKeyHelper",
    "statusLine",
    "awsAuthRefresh",
    "awsCredentialExport",
    "otelHeadersHelper",
    "mcpServers",
    "enabledMcpjsonServers",
    "disabledMcpjsonServers",
];

/// The keys kun's template manages, or `None` when the template cannot be read (then only
/// [`ALWAYS`] edit: the app fails closed).
pub(super) fn managed_keys(kun: &Path) -> Option<BTreeSet<String>> {
    let text = fs::read_to_string(kun.join(".claude/settings.json")).ok()?;
    match serde_json::from_str::<Value>(&text).ok()? {
        Value::Object(m) => Some(m.keys().cloned().collect()),
        _ => None,
    }
}

/// A plain key: letters, digits and `_`, so it is one JSON key and nothing else.
pub(super) fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 64 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether the person may edit `key` here.
pub(super) fn editable(key: &str, managed: Option<&BTreeSet<String>>) -> bool {
    if NEVER.contains(&key) {
        return false;
    }
    ALWAYS.contains(&key) || managed.is_some_and(|m| !m.contains(key))
}

/// A key whose value could be a credential is listed but never shown.
pub(super) fn sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["key", "token", "secret", "password", "credential"]
        .iter()
        .any(|s| lower.contains(s))
}

/// The editable entries of a settings object, sorted by key.
pub(super) fn entries(
    settings: &Map<String, Value>,
    managed: Option<&BTreeSet<String>>,
) -> Vec<SettingEntry> {
    let mut out: Vec<SettingEntry> = settings
        .iter()
        .filter(|(k, _)| valid_key(k) && editable(k, managed))
        .map(|(k, v)| {
            let hidden = sensitive(k);
            SettingEntry {
                key: k.clone(),
                value: if hidden {
                    String::new()
                } else {
                    serde_json::to_string(v).unwrap_or_default()
                },
                hidden,
            }
        })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// Sets (`Some`) or removes (`None`) one key and writes the file, after backing it up. Returns the
/// backup so the caller can restore it if health goes RED.
///
/// # Errors
/// When the key is not editable, the value is not JSON, or a file cannot be read or written.
pub(super) fn edit(
    claude: &Path,
    kun: &Path,
    key: &str,
    value: Option<&str>,
    now: u64,
) -> Result<guard::Backup, ConfigError> {
    if !valid_key(key) {
        return Err(ConfigError::BadKey(key.to_owned()));
    }
    if sensitive(key) || !editable(key, managed_keys(kun).as_ref()) {
        return Err(ConfigError::Managed(key.to_owned()));
    }
    let new_value = value.map(guard::validate_json).transpose()?;
    let path = claude.join("settings.json");
    let mut settings = guard::read_object(&path)?;
    match new_value {
        Some(v) => {
            settings.insert(key.to_owned(), v);
        }
        None => {
            settings.remove(key);
        }
    }
    let text = guard::pretty(&Value::Object(settings));
    // The text we are about to write must read back as an object.
    guard::read_object_text(&text)?;
    let backup = guard::backup(claude, &path, "settings.json", now)?;
    guard::write_atomic(&path, &text)?;
    Ok(backup)
}

/// The one-line `~/.claude/.kun-notify`: 1 to 200 characters, no line break.
///
/// # Errors
/// [`ConfigError::BadLine`] on anything else.
pub(super) fn valid_notify(line: &str) -> Result<String, ConfigError> {
    let line = line.trim();
    if line.is_empty() || line.chars().count() > 200 || line.chars().any(char::is_control) {
        return Err(ConfigError::BadLine);
    }
    Ok(line.to_owned())
}

/// Writes `.kun-notify` after backing it up.
///
/// # Errors
/// On a bad line or a filesystem failure.
pub(super) fn set_notify(
    claude: &Path,
    line: &str,
    now: u64,
) -> Result<guard::Backup, ConfigError> {
    let line = valid_notify(line)?;
    let path = claude.join(".kun-notify");
    let backup = guard::backup(claude, &path, "kun-notify", now)?;
    guard::write_atomic(&path, &format!("{line}\n"))?;
    Ok(backup)
}
