//! config — the types: what the editor shows and what a write reports.

use serde::Serialize;
use ts_rs::TS;

/// One editable top-level key of `~/.claude/settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "config/types.ts")]
pub struct SettingEntry {
    pub key: String,
    /// The value as JSON text; empty when the value is hidden.
    pub value: String,
    /// A value that looks like a secret is never shown and cannot be edited here.
    pub hidden: bool,
}

/// What the Configuration tab loads without the slow parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "config/types.ts")]
pub struct ConfigView {
    /// The line in `~/.claude/.kun-notify`, if the file exists.
    pub notify: Option<String>,
    /// Keys kun's own settings template manages; these are not editable here.
    pub managed: Vec<String>,
    /// `model`, `voice` and every key kun's template does not manage.
    pub settings: Vec<SettingEntry>,
    /// Whether `~/.claude/.env` exists and is not empty. Its contents are never read into the page.
    pub env_present: bool,
    /// Whether the kun checkout has the settings template (otherwise only model and voice edit).
    pub template_found: bool,
}

/// One MCP server `claude mcp list` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "config/types.ts")]
pub struct McpServer {
    pub name: String,
    /// The text after the name, with any key-looking value removed.
    pub summary: String,
    /// `None` when it needs authentication or the state is not known.
    pub connected: Option<bool>,
}

/// A server kun's catalog could add back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "config/types.ts")]
pub struct McpCatalogEntry {
    pub name: String,
    pub description: String,
    /// The catalog marks it as deliberately not registered by setup.
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "config/types.ts")]
pub struct McpView {
    pub installed: Vec<McpServer>,
    /// Catalog servers that are not installed.
    pub available: Vec<McpCatalogEntry>,
}

/// What a guarded write did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "config/types.ts")]
pub struct WriteOutcome {
    pub ok: bool,
    /// The health check went RED, so the backup was restored.
    pub rolled_back: bool,
    /// `green`, `warn`, `red` or `unknown` (health could not run).
    pub verdict: String,
    /// Where the backup of the previous content is.
    pub backup: Option<String>,
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("`{0}` is managed by kun and cannot be edited here")]
    Managed(String),
    #[error("not a valid setting key: {0}")]
    BadKey(String),
    #[error("not valid JSON: {0}")]
    BadJson(String),
    #[error("{0} is not a JSON object")]
    NotObject(String),
    #[error("the line must be 1 to 200 characters with no line break")]
    BadLine,
    #[error("not a valid name: {0}")]
    BadName(String),
    #[error("not a gist id: {0}")]
    BadGist(String),
    #[error("{0} is not in kun's MCP catalog")]
    NotInCatalog(String),
    #[error("{0} is missing")]
    Missing(String),
    #[error("claude is not installed")]
    NoClaude,
    #[error("{0}")]
    Run(String),
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
}

impl Serialize for ConfigError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
