//! repos — where the clones live. The root is a setting: chosen in the app, else adopted from the
//! installer (`reposDir` in its state file), else `~/Kun/repos`. Clones already at `<root>/<name>`
//! (`~/kun`, `~/hogwarts` when the root is `$HOME`) are adopted, never cloned again.
//!
//! Moving the root moves the clones only when nothing would be lost; the same loss check the
//! remove button uses decides. Every function takes its files and folders, so tests run on temp dirs.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use super::{
    git,
    model::{MoveBlock, RepoError, RootInfo, RootSource, io_err},
    ops, state,
};

/// The setting file's name, inside machine's data folder.
pub(super) const SETTING_FILE: &str = "repos-root.json";

/// `reposDir` from the installer's state file: an absolute path, else nothing.
pub(super) fn adopt_from_installer(state_file: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(state_file).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let dir = PathBuf::from(value.get("reposDir")?.as_str()?.trim());
    (dir.is_absolute() && dir.parent().is_some()).then_some(dir)
}

/// Where the installer leaves its state: `~/Library/Application Support/Databayt/` on macOS,
/// `$XDG_CONFIG_HOME/databayt/` (else `~/.config/databayt/`) on Linux. Windows has no installer file.
pub(super) fn installer_state_path(home: &Path, xdg_config: Option<&Path>) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support/Databayt/installer-state.json"))
    } else if cfg!(target_os = "linux") {
        let base = xdg_config.map_or_else(|| home.join(".config"), Path::to_path_buf);
        Some(base.join("databayt/installer-state.json"))
    } else {
        None
    }
}

/// The saved choice, if there is one.
pub(super) fn load_setting(file: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(file).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let dir = PathBuf::from(value.get("root")?.as_str()?);
    dir.is_absolute().then_some(dir)
}

/// Saves the choice atomically.
pub(super) fn save_setting(file: &Path, root: &Path) -> Result<(), RepoError> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let tmp = file.with_extension("json.tmp");
    let text =
        serde_json::to_string_pretty(&serde_json::json!({ "root": root.display().to_string() }))
            .unwrap_or_default();
    fs::write(&tmp, text).map_err(io_err(&tmp))?;
    fs::rename(&tmp, file).map_err(io_err(file))
}

/// The root now in force: the saved choice, else the installer's, else `default`.
pub(super) fn resolve(
    setting_file: &Path,
    installer_file: Option<&Path>,
    default: &Path,
) -> (PathBuf, RootSource) {
    if let Some(dir) = load_setting(setting_file) {
        return (dir, RootSource::Setting);
    }
    if let Some(dir) = installer_file.and_then(adopt_from_installer) {
        return (dir, RootSource::Installer);
    }
    (default.to_path_buf(), RootSource::Default)
}

/// A folder this app may keep clones in: absolute, no `..`, not the filesystem root, not a file.
///
/// # Errors
/// [`RepoError::BadRoot`] with the reason.
pub(super) fn validate(path: &Path) -> Result<PathBuf, RepoError> {
    let bad = |why: &str| RepoError::BadRoot(format!("{} ({why})", path.display()));
    if !path.is_absolute() {
        return Err(bad("not an absolute path"));
    }
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(bad("contains . or .."));
    }
    if path.parent().is_none() {
        return Err(bad("the filesystem root"));
    }
    if path.exists() && !path.is_dir() {
        return Err(bad("a file"));
    }
    Ok(path.components().collect())
}

/// Everything about the root in force.
pub(super) fn info(setting_file: &Path, installer_file: Option<&Path>, default: &Path) -> RootInfo {
    let (root, source) = resolve(setting_file, installer_file, default);
    RootInfo {
        clones: u32::try_from(ops::local_paths(&root).len()).unwrap_or(u32::MAX),
        root: root.display().to_string(),
        source,
        default_root: default.display().to_string(),
    }
}

/// The clones a move carries: all of them, except the `pinned` folder (kun's own checkout, which the
/// engine finds by its path and which therefore stays where it is).
fn movable(old: &Path, pinned: Option<&Path>) -> Vec<PathBuf> {
    ops::local_paths(old)
        .into_iter()
        .filter(|d| pinned != Some(d.as_path()))
        .collect()
}

/// The clones under `old` that block a move to `new`: work that would be lost, or a folder already
/// at the destination. Empty means every clone can move.
///
/// # Errors
/// When git cannot be asked.
pub(super) fn move_blockers(
    old: &Path,
    new: &Path,
    pinned: Option<&Path>,
) -> Result<Vec<MoveBlock>, RepoError> {
    let mut blocks = Vec::new();
    for dir in movable(old, pinned) {
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let mut losses = git::losses(&dir)?;
        if new.join(&name).exists() {
            // Not lost work, but the move cannot proceed: shown with no loss kinds.
            losses.clear();
        } else if losses.is_empty() {
            continue;
        }
        blocks.push(MoveBlock { name, losses });
    }
    Ok(blocks)
}

/// Moves every clone from `old` to `new` and carries their state, or moves none.
///
/// # Errors
/// [`RepoError::MoveRefused`] naming the blocking clones; an io error when a rename fails (for
/// instance across volumes, where the clones stay where they were).
pub(super) fn move_clones(
    old: &Path,
    new: &Path,
    pinned: Option<&Path>,
) -> Result<Vec<String>, RepoError> {
    let blocks = move_blockers(old, new, pinned)?;
    if !blocks.is_empty() {
        return Err(RepoError::MoveRefused(
            blocks.into_iter().map(|b| b.name).collect(),
        ));
    }
    fs::create_dir_all(new).map_err(io_err(new))?;
    let mut moved: Vec<String> = Vec::new();
    for dir in movable(old, pinned) {
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if let Err(e) = fs::rename(&dir, new.join(&name)) {
            // Put back what already moved so a half-move never happens.
            for done in &moved {
                let _ = fs::rename(new.join(done), old.join(done));
            }
            return Err(io_err(&dir)(e));
        }
        moved.push(name);
    }
    let mut old_state = state::load(old);
    let carried: state::State = moved
        .iter()
        .filter_map(|n| old_state.remove(n).map(|e| (n.clone(), e)))
        .collect();
    if !carried.is_empty() {
        let mut merged = state::load(new);
        merged.extend(carried);
        state::save(new, &merged)?;
        state::save(old, &old_state)?;
    }
    Ok(moved)
}

/// Switches the root, optionally moving the clones first. The setting is written last, so a failed
/// move leaves the old root in force.
///
/// # Errors
/// On a root that is not allowed, a refused or failed move, or an unwritable setting.
pub(super) fn set_root(
    setting_file: &Path,
    installer_file: Option<&Path>,
    default: &Path,
    new_root: &Path,
    move_existing: bool,
    pinned: Option<&Path>,
) -> Result<RootInfo, RepoError> {
    let new = validate(new_root)?;
    let (old, _) = resolve(setting_file, installer_file, default);
    if new == old {
        // Nothing moves; the choice is still saved, so it no longer follows the installer.
    } else if move_existing {
        move_clones(&old, &new, pinned)?;
    } else {
        fs::create_dir_all(&new).map_err(io_err(&new))?;
    }
    save_setting(setting_file, &new)?;
    Ok(info(setting_file, installer_file, default))
}
