//! config — the Configuration tab: the few things a person tunes on their own computer.
//!
//! Editable: the profile's department (the settings feature), `~/.claude/.kun-notify`, the
//! top-level keys of `~/.claude/settings.json` kun does not manage, and the MCP servers. Every write
//! backs up first, validates, writes atomically, runs `health.sh --json` and restores the backup if
//! the verdict goes RED. Secrets are never shown: `~/.claude/.env` is only "present" or "absent",
//! and loading it from the team gist runs kun's `secrets.sh` in Terminal.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::process::Command;

#[path = "guard.rs"]
mod guard;
#[path = "mcp.rs"]
mod mcp;
#[path = "model.rs"]
pub mod model;
#[path = "settings.rs"]
mod settings;

use model::{ConfigError, ConfigView, McpView, WriteOutcome};

/// `claude mcp list` health-checks every server, which can take a while.
const MCP_LIST_LIMIT: Duration = Duration::from_secs(120);
const MCP_EDIT_LIMIT: Duration = Duration::from_secs(60);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `~/.claude.json`, beside the claude folder.
fn claude_json(claude: &Path) -> PathBuf {
    claude.with_file_name(".claude.json")
}

/// Whether `<claude>/.env` exists and has content. Its content is never read.
fn env_present(claude: &Path) -> bool {
    std::fs::metadata(claude.join(".env")).is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// The Configuration tab's first load.
pub(super) fn load_view(claude: &Path, kun: &Path) -> Result<ConfigView, ConfigError> {
    let managed = settings::managed_keys(kun);
    let all = guard::read_object(&claude.join("settings.json"))?;
    let notify = std::fs::read_to_string(claude.join(".kun-notify"))
        .ok()
        .map(|t| t.lines().next().unwrap_or("").trim().to_owned());
    Ok(ConfigView {
        notify,
        managed: managed.iter().flatten().cloned().collect(),
        settings: settings::entries(&all, managed.as_ref()),
        env_present: env_present(claude),
        template_found: managed.is_some(),
    })
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ConfigError> + Send + 'static,
) -> Result<T, ConfigError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ConfigError::Run(e.to_string()))?
}

/// `config_load` — notify line, editable settings, whether secrets are present.
#[tauri::command]
async fn config_load() -> Result<ConfigView, ConfigError> {
    let (claude, kun) = (crate::engine::claude_dir(), crate::engine::kun_dir());
    blocking(move || load_view(&claude, &kun)).await
}

/// `config_set_notify` — writes the one-line `~/.claude/.kun-notify`, guarded.
#[tauri::command]
async fn config_set_notify(text: String) -> Result<WriteOutcome, ConfigError> {
    settings::valid_notify(&text)?;
    let (claude, kun) = (crate::engine::claude_dir(), crate::engine::kun_dir());
    let before = guard::health_verdict(&kun).await;
    let backup = {
        let claude = claude.clone();
        blocking(move || settings::set_notify(&claude, &text, now_secs())).await?
    };
    let after = guard::health_verdict(&kun).await;
    guard::finish(&backup, before, after, guard::Backup::restore)
}

/// `config_set_setting` — sets one editable key of `~/.claude/settings.json` to a JSON value, guarded.
#[tauri::command]
async fn config_set_setting(key: String, value: String) -> Result<WriteOutcome, ConfigError> {
    edit_setting(key, Some(value)).await
}

/// `config_remove_setting` — removes one editable key, guarded.
#[tauri::command]
async fn config_remove_setting(key: String) -> Result<WriteOutcome, ConfigError> {
    edit_setting(key, None).await
}

async fn edit_setting(key: String, value: Option<String>) -> Result<WriteOutcome, ConfigError> {
    let (claude, kun) = (crate::engine::claude_dir(), crate::engine::kun_dir());
    let before = guard::health_verdict(&kun).await;
    let backup = {
        let (claude, kun) = (claude.clone(), kun.clone());
        blocking(move || settings::edit(&claude, &kun, &key, value.as_deref(), now_secs())).await?
    };
    let after = guard::health_verdict(&kun).await;
    guard::finish(&backup, before, after, guard::Backup::restore)
}

fn claude_binary() -> Result<PathBuf, ConfigError> {
    crate::driver::detect()
        .into_iter()
        .find(|d| d.kind == crate::driver::DriverKind::Claude && d.installed)
        .and_then(|d| d.path)
        .map(PathBuf::from)
        .ok_or(ConfigError::NoClaude)
}

async fn run_claude(args: &[String], limit: Duration) -> Result<String, ConfigError> {
    let run = Command::new(claude_binary()?)
        .args(args)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(limit, run)
        .await
        .map_err(|_| ConfigError::Run("claude took too long".into()))?
        .map_err(|e| ConfigError::Run(e.to_string()))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(text)
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(ConfigError::Run(
            format!("{} {}", text.trim(), err.trim())
                .trim()
                .chars()
                .take(300)
                .collect(),
        ))
    }
}

/// `config_mcp_list` — the installed MCP servers (from `claude mcp list`, slow) and the catalog
/// servers that could be added back.
#[tauri::command]
async fn config_mcp_list() -> Result<McpView, ConfigError> {
    let installed =
        mcp::parse_list(&run_claude(&["mcp".into(), "list".into()], MCP_LIST_LIMIT).await?);
    Ok(mcp::view(&crate::engine::kun_dir(), installed))
}

async fn mcp_change(args: Vec<String>) -> Result<WriteOutcome, ConfigError> {
    let (claude, kun) = (crate::engine::claude_dir(), crate::engine::kun_dir());
    let before = guard::health_verdict(&kun).await;
    let backup = {
        let (claude, file) = (claude.clone(), claude_json(&claude));
        blocking(move || guard::backup(&claude, &file, "claude.json", now_secs())).await?
    };
    if let Err(e) = run_claude(&args, MCP_EDIT_LIMIT).await {
        // The call failed: put back only the servers, in case it half-applied.
        let _ = backup.restore_key("mcpServers");
        return Err(e);
    }
    let after = guard::health_verdict(&kun).await;
    guard::finish(&backup, before, after, |b| b.restore_key("mcpServers"))
}

/// `config_mcp_remove` — `claude mcp remove <name> -s user`, guarded.
#[tauri::command]
async fn config_mcp_remove(name: String) -> Result<WriteOutcome, ConfigError> {
    mcp_change(mcp::remove_args(&name)?).await
}

/// `config_mcp_add` — re-adds a server from kun's `.claude/mcp.json` with `claude mcp add-json`, guarded.
#[tauri::command]
async fn config_mcp_add(name: String) -> Result<WriteOutcome, ConfigError> {
    mcp_change(mcp::add_args(&crate::engine::kun_dir(), &name)?).await
}

/// A gist id is hex-ish letters and digits.
pub(super) fn valid_gist(id: &str) -> bool {
    (6..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The Terminal line that loads the team secrets: the id is typed by the person into the app, the
/// secrets are fetched by kun's script into `~/.claude/.env`, and nothing comes back here.
pub(super) fn secrets_line(kun: &Path, gist_id: &str) -> Result<String, ConfigError> {
    if !valid_gist(gist_id) {
        return Err(ConfigError::BadGist(gist_id.chars().take(12).collect()));
    }
    Ok(format!(
        "cd {} && bash .claude/scripts/secrets.sh {gist_id}",
        crate::readiness::shell_quote(&kun.display().to_string())
    ))
}

/// `config_load_secrets` — runs `secrets.sh <GIST_ID>` in Terminal.
#[tauri::command]
async fn config_load_secrets(gist_id: String) -> Result<(), ConfigError> {
    let line = secrets_line(&crate::engine::kun_dir(), gist_id.trim())?;
    crate::readiness::run_in_terminal(&line)
        .await
        .map_err(ConfigError::Run)
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("config")
        .invoke_handler(tauri::generate_handler![
            config_load,
            config_set_notify,
            config_set_setting,
            config_remove_setting,
            config_mcp_list,
            config_mcp_remove,
            config_mcp_add,
            config_load_secrets
        ])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
