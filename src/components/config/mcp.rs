//! config — MCP servers: what `claude mcp list` reports, what kun's catalog could add back, and the
//! arguments of the two `claude mcp` calls machine makes. No shell: every call is a vector.

use std::{collections::BTreeSet, fs, path::Path};

use serde_json::Value;

use super::model::{ConfigError, McpCatalogEntry, McpServer, McpView};

/// A server name: what `claude mcp` accepts, nothing a flag could use.
pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
}

/// Whether a word could be a credential: long and mixed, or after a `Bearer`/`key=`.
fn secret_like(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    w.len() >= 24
        && w.chars().any(|c| c.is_ascii_digit())
        && w.chars().any(|c| c.is_ascii_alphabetic())
        && !w.contains('/')
}

/// Takes secrets out of a summary line: `KEY=value` pairs and long token-looking words.
pub(super) fn redact(text: &str) -> String {
    text.split(' ')
        .map(|w| {
            if let Some((k, _)) = w.split_once('=') {
                let lower = k.to_ascii_lowercase();
                if ["key", "token", "secret", "password", "auth"]
                    .iter()
                    .any(|s| lower.contains(s))
                {
                    return format!("{k}=…");
                }
            }
            if secret_like(w) {
                "…".to_owned()
            } else {
                w.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `claude mcp list` lines: `name: command - ✓ Connected`. Anything else is ignored.
pub(super) fn parse_list(output: &str) -> Vec<McpServer> {
    output
        .lines()
        .filter(|l| !l.starts_with(char::is_whitespace))
        .filter_map(|line| {
            let (name, rest) = line.split_once(": ")?;
            let name = name.trim();
            if !valid_name(name) {
                return None;
            }
            let connected = if rest.contains('✓') {
                Some(true)
            } else if rest.contains('✗') {
                Some(false)
            } else {
                None
            };
            let summary = redact(rest.trim()).chars().take(200).collect();
            Some(McpServer {
                name: name.to_owned(),
                summary,
                connected,
            })
        })
        .collect()
}

/// `<kun>/.claude/mcp.json` → `mcpServers`, or nothing.
fn catalog(kun: &Path) -> serde_json::Map<String, Value> {
    fs::read_to_string(kun.join(".claude/mcp.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| match v.get("mcpServers") {
            Some(Value::Object(m)) => Some(m.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The installed servers plus the catalog entries that are not installed.
pub(super) fn view(kun: &Path, installed: Vec<McpServer>) -> McpView {
    let have: BTreeSet<&str> = installed.iter().map(|s| s.name.as_str()).collect();
    let available = catalog(kun)
        .iter()
        .filter(|(name, _)| valid_name(name) && !have.contains(name.as_str()))
        .map(|(name, entry)| McpCatalogEntry {
            name: name.clone(),
            description: entry
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            skipped: entry
                .get("$skipRegistration")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect();
    McpView {
        installed,
        available,
    }
}

/// `claude mcp remove <name> -s user`.
///
/// # Errors
/// On a name `claude` could mistake for a flag.
pub(super) fn remove_args(name: &str) -> Result<Vec<String>, ConfigError> {
    if !valid_name(name) {
        return Err(ConfigError::BadName(name.to_owned()));
    }
    Ok(["mcp", "remove", name, "-s", "user"]
        .map(String::from)
        .to_vec())
}

/// `claude mcp add-json -s user <name> <json>` from the catalog entry, without the catalog's own
/// notes (`description` and every `$…` key).
///
/// # Errors
/// On a bad name or a name the catalog does not have.
pub(super) fn add_args(kun: &Path, name: &str) -> Result<Vec<String>, ConfigError> {
    if !valid_name(name) {
        return Err(ConfigError::BadName(name.to_owned()));
    }
    let entry = catalog(kun)
        .get(name)
        .cloned()
        .ok_or_else(|| ConfigError::NotInCatalog(name.to_owned()))?;
    let Value::Object(map) = entry else {
        return Err(ConfigError::NotInCatalog(name.to_owned()));
    };
    let clean: serde_json::Map<String, Value> = map
        .into_iter()
        .filter(|(k, _)| k != "description" && !k.starts_with('$'))
        .collect();
    let json = serde_json::to_string(&Value::Object(clean))
        .map_err(|e| ConfigError::BadJson(e.to_string()))?;
    Ok(["mcp", "add-json", "-s", "user", name, &json]
        .map(String::from)
        .to_vec())
}
