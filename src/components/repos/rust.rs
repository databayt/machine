//! repos — the databayt repos on this computer, cloned into `<root>/<name>`. The root is a setting
//! (see `root.rs`): chosen in the app, else the installer's `reposDir`, else `~/Kun/repos`; clones
//! already there are adopted, never cloned again.
//!
//! Clone is shallow (50) unless asked for full, pull is `--ff-only` and nothing else, and removal
//! refuses when it would lose work. machine never pushes, never commits and never stores a git
//! credential. The state (`keep` or `sprint` per clone) lives in machine's data folder, never in the root.
//! `pack` calls [`sync_all`] to sync a profile's repos; agent chat reads [`local_paths`] for `--add-dir`.

use std::path::PathBuf;

use tauri::{
    Runtime,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
};

#[path = "git.rs"]
mod git;
#[path = "model.rs"]
mod model;
#[path = "ops.rs"]
mod ops;
#[path = "root.rs"]
mod root;
#[path = "state.rs"]
mod state;

pub use model::LocalRepo;
pub use model::{
    Loss, LossKind, MoveBlock, OrgRepoArg, RepoEntry, RepoError, RepoFailure, RepoMode, RepoRow,
    RootInfo, RootSource, SweepReport,
};
pub use ops::{GITHUB_BASE, SyncEvent, SyncState, local_paths, local_repos, repos_root, sync_all};

/// Kun's own checkout, which a root move leaves where it is.
fn pinned_kun() -> PathBuf {
    crate::engine::kun_dir()
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, RepoError> + Send + 'static,
) -> Result<T, RepoError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| RepoError::Git(e.to_string()))?
}

/// `list` — the org list TS passes in (or the profile's repos when it is empty) merged with what is
/// on disk. `fetch` refreshes ahead/behind from the network first.
#[tauri::command]
async fn list(org: Vec<OrgRepoArg>, fetch: Option<bool>) -> Result<Vec<RepoRow>, RepoError> {
    let root = repos_root()?;
    let profile_repos = if org.is_empty() {
        crate::profile::current()
            .map(|r| r.profile.repos)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    blocking(move || {
        Ok(ops::rows(
            &root,
            &org,
            &profile_repos,
            fetch.unwrap_or(false),
        ))
    })
    .await
}

/// `clone` — `depth` is 50 when omitted, `0` for the full history. Progress streams over `channel`.
#[tauri::command]
async fn clone(
    name: String,
    depth: Option<u32>,
    channel: Channel<SyncEvent>,
) -> Result<(), RepoError> {
    ops::clone_or_adopt_in(GITHUB_BASE, &repos_root()?, &name, depth, true, &mut |e| {
        let _ = channel.send(e);
    })
    .await
}

/// `pull` — `git pull --ff-only`, streaming progress.
#[tauri::command]
async fn pull(name: String, channel: Channel<SyncEvent>) -> Result<(), RepoError> {
    ops::pull_in(&repos_root()?, &name, true, &mut |e| {
        let _ = channel.send(e);
    })
    .await
}

/// `set_mode` — `keep`, or `sprint` with the sprint's name.
#[tauri::command]
async fn set_mode(name: String, mode: RepoMode, sprint: Option<String>) -> Result<(), RepoError> {
    let root = repos_root()?;
    blocking(move || ops::set_mode_in(&root, &name, mode, sprint)).await
}

/// `remove` — deletes the clone, or rejects with the typed list of what would be lost.
#[tauri::command]
async fn remove(name: String) -> Result<(), RepoError> {
    let root = repos_root()?;
    blocking(move || ops::remove_in(&root, &name)).await
}

/// `set_remote` — only `https://github.com/databayt/<name>[.git]`.
#[tauri::command]
async fn set_remote(name: String, url: String) -> Result<(), RepoError> {
    let root = repos_root()?;
    blocking(move || ops::set_remote_in(&root, &name, &url)).await
}

/// `open_remote` — the repo's GitHub URL; the page opens it.
#[tauri::command]
async fn open_remote(name: String) -> Result<String, RepoError> {
    let root = repos_root()?;
    blocking(move || ops::remote_url_in(&root, &name)).await
}

/// `sweep_sprint` — removes the clean, pushed sprint clones whose sprint is over.
#[tauri::command]
async fn sweep_sprint(current_sprint: String) -> Result<SweepReport, RepoError> {
    let root = repos_root()?;
    blocking(move || ops::sweep_in(&root, &current_sprint)).await
}

/// `get_root` — where the clones live and how that was decided.
#[tauri::command]
async fn get_root() -> Result<RootInfo, RepoError> {
    let default = ops::default_root()?;
    let setting = ops::setting_file()?;
    blocking(move || Ok(root::info(&setting, ops::installer_file().as_deref(), &default))).await
}

/// `move_blockers` — the clones that would stop a move to `path` (unsaved work, a name already
/// there). Empty means the move is safe.
#[tauri::command]
async fn move_blockers(path: String) -> Result<Vec<MoveBlock>, RepoError> {
    let new = root::validate(std::path::Path::new(&path))?;
    let old = repos_root()?;
    blocking(move || root::move_blockers(&old, &new, Some(pinned_kun().as_path()))).await
}

/// `set_root` — switches the repos root. With `move_clones` the clones follow, but only when none
/// would lose work; without it they stay where they are and the new root starts empty (clones
/// already in it are adopted).
#[tauri::command]
async fn set_root(path: String, move_clones: bool) -> Result<RootInfo, RepoError> {
    let default = ops::default_root()?;
    let setting = ops::setting_file()?;
    blocking(move || {
        root::set_root(
            &setting,
            ops::installer_file().as_deref(),
            &default,
            std::path::Path::new(&path),
            move_clones,
            Some(pinned_kun().as_path()),
        )
    })
    .await
}

/// The clones' folders, for callers that need them as paths (agent chat's `--add-dir`).
#[must_use]
pub fn local_dirs() -> Vec<PathBuf> {
    repos_root().map(|r| local_paths(&r)).unwrap_or_default()
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("repos")
        .invoke_handler(tauri::generate_handler![
            list,
            clone,
            pull,
            set_mode,
            remove,
            set_remote,
            open_remote,
            sweep_sprint,
            get_root,
            move_blockers,
            set_root
        ])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
