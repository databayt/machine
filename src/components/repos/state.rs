//! The state file: `{ name → { mode, sprint, addedAt } }` per repos root.
//!
//! It lives in machine's own data folder (`~/.kun/machine/repos-state-<root hash>.json`), never in
//! the repos root itself: that root may be `$HOME`, which must not collect dotfiles. An older
//! `<root>/.kun-repos.json` is read when no new file exists yet. Under test the root holds it, so
//! tests stay on their temp folders.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use super::model::{RepoEntry, RepoError, RepoMode, io_err};

pub(super) const STATE_FILE: &str = ".kun-repos.json";

pub(super) type State = BTreeMap<String, RepoEntry>;

/// Where the state of `root` lives under machine's data folder `app`.
pub(super) fn file_in(app: &Path, root: &Path) -> PathBuf {
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    let key = digest.iter().take(6).fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    app.join(format!("repos-state-{key}.json"))
}

fn app_dir() -> Option<PathBuf> {
    if cfg!(test) {
        None
    } else {
        crate::pack::machine_dir().ok()
    }
}

/// Reads the state; a missing or unreadable file is an empty state, never an error.
pub(super) fn load_with(app: Option<&Path>, root: &Path) -> State {
    let read = |p: &Path| {
        fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str::<State>(&s).ok())
    };
    app.and_then(|a| read(&file_in(a, root)))
        .or_else(|| read(&root.join(STATE_FILE)))
        .unwrap_or_default()
}

/// Reads the state of `root`.
pub(super) fn load(root: &Path) -> State {
    load_with(app_dir().as_deref(), root)
}

/// Writes the state atomically (temp file, then rename).
pub(super) fn save_with(app: Option<&Path>, root: &Path, state: &State) -> Result<(), RepoError> {
    let (dir, file) = match app {
        Some(a) => (a.to_path_buf(), file_in(a, root)),
        None => (root.to_path_buf(), root.join(STATE_FILE)),
    };
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let tmp = file.with_extension("json.tmp");
    let mut text = serde_json::to_string_pretty(state).unwrap_or_else(|_| "{}".into());
    text.push('\n');
    fs::write(&tmp, text).map_err(io_err(&tmp))?;
    fs::rename(&tmp, &file).map_err(io_err(&file))
}

/// Writes the state of `root`.
pub(super) fn save(root: &Path, state: &State) -> Result<(), RepoError> {
    save_with(app_dir().as_deref(), root, state)
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Records a repo as `keep` unless it is already known.
pub(super) fn ensure(root: &Path, name: &str) -> Result<(), RepoError> {
    let mut state = load(root);
    if !state.contains_key(name) {
        state.insert(
            name.to_owned(),
            RepoEntry {
                mode: RepoMode::Keep,
                sprint: None,
                added_at: now(),
            },
        );
        save(root, &state)?;
    }
    Ok(())
}
