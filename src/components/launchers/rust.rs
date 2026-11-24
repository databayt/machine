//! launchers — the agent launchers (`c` Claude, `a` Antigravity, `o` opencode, `h` Hermes) in the
//! shell, set from Machine.
//!
//! kun owns the list (`<kun>/.claude/machine/launchers.json`) and the block markers; this feature
//! reads the list, says which launchers are installed and which are in the managed block, and
//! rewrites the block in `~/.zshrc` and `~/.bashrc` (Mac, Linux) or the `PowerShell` `$PROFILE`
//! (Windows) with the same logic as kun's `lib/agents.sh`. Lines outside the markers are kept, the
//! first change to a file leaves a `.kun-backup` beside it, and a new terminal picks it up.
//!
//! Commands: `launchers_status`, `launchers_apply`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;

#[path = "block.rs"]
mod block;

use block::{Launcher, Shell};

/// `<kun>/.claude/machine/launchers.json`.
fn catalog_path(kun: &Path) -> PathBuf {
    kun.join(".claude/machine/launchers.json")
}

/// One launcher and where it stands on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "launchers/types.ts")]
pub struct LauncherRow {
    pub id: String,
    pub name: String,
    /// The exact command the shell runs, bypass flag included.
    pub command: String,
    pub bypass: bool,
    pub bypass_note: Option<String>,
    /// The program is on this computer.
    pub installed: bool,
    /// The launcher is in the managed block.
    pub enabled: bool,
    /// The readiness step that installs the program.
    pub needs: Option<String>,
}

/// Everything the Agents settings page draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "launchers/types.ts")]
pub struct LaunchersView {
    /// False when kun has no launchers file on this computer yet.
    pub found: bool,
    pub rows: Vec<LauncherRow>,
    /// The shell files the block lives in (`~/.zshrc`, …).
    pub files: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    #[error("kun has no launchers file here ({0}); update the kun checkout")]
    NoCatalog(String),
    #[error("the launchers file is damaged: {0}")]
    Bad(String),
    #[error("not a launcher: {0}")]
    Unknown(String),
    #[error("could not write {0}: {1}")]
    Write(String, String),
    #[error("{0}")]
    Other(String),
}

impl Serialize for LauncherError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

fn load_launchers() -> Result<Vec<Launcher>, LauncherError> {
    let path = catalog_path(&crate::engine::kun_dir());
    let text = fs::read_to_string(&path)
        .map_err(|_| LauncherError::NoCatalog(path.display().to_string()))?;
    block::parse(&text).map_err(LauncherError::Bad)
}

/// A launcher as the terminal runs it: the program and its fixed arguments, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LauncherSpec {
    pub id: String,
    pub name: String,
    pub bin: String,
    pub args: Vec<String>,
}

/// The launchers kun lists (`c`, `a`, `o`, `h`, `f`, …), in catalog order.
///
/// # Errors
/// When kun has no launchers file here or it is damaged.
pub(crate) fn launcher_specs() -> Result<Vec<LauncherSpec>, String> {
    load_launchers().map_err(|e| e.to_string()).map(|all| {
        all.into_iter()
            .map(|l| LauncherSpec { id: l.id, name: l.name, bin: l.bin, args: l.args })
            .collect()
    })
}

fn shell() -> Shell {
    if cfg!(windows) { Shell::PowerShell } else { Shell::Posix }
}

/// The files the block lives in: the rc files that exist, else the one a new terminal reads.
fn targets(home: &Path, shell: Shell, documents: Option<&Path>) -> Vec<PathBuf> {
    let candidates: Vec<PathBuf> = match shell {
        Shell::Posix => vec![home.join(".zshrc"), home.join(".bashrc")],
        Shell::PowerShell => {
            let docs = documents.map_or_else(|| home.join("Documents"), Path::to_path_buf);
            vec![
                docs.join("PowerShell/Microsoft.PowerShell_profile.ps1"),
                docs.join("WindowsPowerShell/Microsoft.PowerShell_profile.ps1"),
            ]
        }
    };
    let existing: Vec<PathBuf> = candidates.iter().filter(|p| p.is_file()).cloned().collect();
    if !existing.is_empty() {
        return existing;
    }
    let first = match shell {
        Shell::Posix if cfg!(target_os = "macos") => candidates[0].clone(),
        Shell::Posix => candidates[1].clone(),
        Shell::PowerShell => candidates[0].clone(),
    };
    vec![first]
}

fn real_targets() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    targets(&home, shell(), dirs::document_dir().as_deref())
}

fn status_with(launchers: &[Launcher], files: &[PathBuf], installed: impl Fn(&str) -> bool) -> LaunchersView {
    let enabled: Vec<String> = files
        .iter()
        .filter_map(|f| fs::read_to_string(f).ok())
        .flat_map(|t| block::enabled_in(&t, launchers, shell()))
        .collect();
    LaunchersView {
        found: true,
        rows: launchers
            .iter()
            .map(|l| LauncherRow {
                id: l.id.clone(),
                name: l.name.clone(),
                command: l.command(),
                bypass: l.bypass,
                bypass_note: l.bypass_note.clone(),
                installed: installed(&l.bin),
                enabled: enabled.contains(&l.id),
                needs: l.needs.clone(),
            })
            .collect(),
        files: files.iter().map(|f| f.display().to_string()).collect(),
    }
}

fn installed(bin: &str) -> bool {
    crate::driver::find_binary(bin, &crate::driver::search_dirs()).is_some()
}

fn view_now() -> LaunchersView {
    match load_launchers() {
        Ok(l) => status_with(&l, &real_targets(), installed),
        Err(_) => LaunchersView {
            found: false,
            rows: Vec::new(),
            files: Vec::new(),
        },
    }
}

/// Rewrites the block in each file for `enabled`; a changed file is backed up once first.
fn write_blocks(launchers: &[Launcher], enabled: &[String], files: &[PathBuf], shell: Shell) -> Result<(), LauncherError> {
    for file in files {
        let old = fs::read_to_string(file).unwrap_or_default();
        let new = block::splice(&old, launchers, enabled, shell);
        if new == old {
            continue;
        }
        let fail = |e: std::io::Error| LauncherError::Write(file.display().to_string(), e.to_string());
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(fail)?;
        }
        if file.is_file() {
            let mut backup = file.clone().into_os_string();
            backup.push(".kun-backup");
            let backup = PathBuf::from(backup);
            if !backup.exists() {
                fs::copy(file, &backup).map_err(fail)?;
            }
        }
        // Written in place, not renamed over, so the file keeps its permissions and symlinks.
        fs::write(file, new).map_err(fail)?;
    }
    Ok(())
}

/// Sets opencode's config-level bypass, the way kun's `agents_configure_opencode` does.
fn allow_opencode(home: &Path) -> Result<(), LauncherError> {
    let path = home.join(".config/opencode/opencode.json");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let Some(new) = block::opencode_allow(&text).map_err(LauncherError::Other)? else {
        return Ok(());
    };
    let fail = |e: std::io::Error| LauncherError::Write(path.display().to_string(), e.to_string());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    fs::write(&path, new).map_err(fail)
}

fn apply_with(
    launchers: &[Launcher],
    enabled: &[String],
    files: &[PathBuf],
    shell: Shell,
    home: &Path,
) -> Result<(), LauncherError> {
    if let Some(bad) = enabled.iter().find(|id| !launchers.iter().any(|l| &l.id == *id)) {
        return Err(LauncherError::Unknown(bad.clone()));
    }
    write_blocks(launchers, enabled, files, shell)?;
    if enabled.iter().any(|id| id == "o") {
        allow_opencode(home)?;
    }
    Ok(())
}

/// `launchers_status` — each launcher: its command, whether the program is installed, whether it is
/// in the block.
#[tauri::command]
async fn launchers_status() -> Result<LaunchersView, LauncherError> {
    tokio::task::spawn_blocking(view_now)
        .await
        .map_err(|e| LauncherError::Other(e.to_string()))
}

/// `launchers_apply` — makes `enabled` (launcher ids) the managed block, in every shell file, and
/// sets opencode's `"permission": "allow"` when `o` is on. Returns the new state.
#[tauri::command]
async fn launchers_apply(enabled: Vec<String>) -> Result<LaunchersView, LauncherError> {
    tokio::task::spawn_blocking(move || {
        let launchers = load_launchers()?;
        let home = dirs::home_dir().ok_or_else(|| LauncherError::Other("no home folder".into()))?;
        apply_with(&launchers, &enabled, &real_targets(), shell(), &home)?;
        Ok(view_now())
    })
    .await
    .map_err(|e| LauncherError::Other(e.to_string()))?
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("launchers")
        .invoke_handler(tauri::generate_handler![launchers_status, launchers_apply])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
