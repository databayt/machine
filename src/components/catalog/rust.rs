//! catalog — browse, install and uninstall kun's skills, agents, rules, hooks and department packs,
//! see what is really under `~/.claude` (`tree`), and turn skills, agents and rules off and on
//! without deleting them (`disable`, `enable`).
//!
//! The registry (`public/r/kun/index.json`, built by `pnpm kun:registry`) lists items and the
//! sha256 of every file. Install never takes content from the webview: it copies from the local
//! kun checkout, verifies each file against the registry hash, and writes atomically into
//! `~/.claude`. What was installed is recorded in `~/.kun/machine/installed.json` so uninstall
//! removes exactly those files and nothing the teammate made themselves.
//!
//! A unit switched off is moved to `<kun_home>/machine/disabled/` (same relative path) and noted in
//! `<kun_home>/machine/disabled.json`, which kun's `setup.sh` reads too: it must not reinstall what
//! a person disabled or removed on purpose.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;

use crate::profile::{CatalogAccess, MachineProfile};

/// Where an item stands on this computer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "catalog/types.ts")]
pub enum ItemStatus {
    Installed,
    #[default]
    Available,
    Outdated,
}

/// One file of an item: its place in the kun repo, its place under `~/.claude`, its hash.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "catalog/types.ts")]
pub struct CatalogFile {
    pub path: String,
    pub target: String,
    pub sha256: String,
}

/// A registry item plus its status here. `id` is `<type>:<name>`, e.g. `skill:funnel`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "catalog/types.ts")]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Arabic title and description, when the source carries them (department packs do).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title_ar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description_ar: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub files: Vec<CatalogFile>,
    pub version: String,
    #[serde(default)]
    pub status: ItemStatus,
    #[serde(default)]
    pub installed_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Registry {
    items: Vec<CatalogItem>,
}

/// One lockfile entry: what this item put on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Installed {
    version: String,
    files: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Lock {
    #[serde(default)]
    items: BTreeMap<String, Installed>,
}

/// What a unit under `~/.claude` is for this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "catalog/types.ts")]
pub enum EntryState {
    /// kun's, on disk, every file as the registry has it.
    Installed,
    /// kun's, on disk, but a file is missing or differs from the registry.
    Outdated,
    /// kun's, not on disk.
    Available,
    /// Moved aside by a person; waiting in the disabled store.
    Disabled,
    /// On disk and not in the registry: made by the person.
    Personal,
}

/// Whose unit it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "catalog/types.ts")]
pub enum Owner {
    /// The registry lists it.
    Kun,
    /// Made by the person.
    You,
}

/// One unit of the real `~/.claude` tree, merged with the registry. `id` is `<kind>:<name>`
/// (`skill`, `agent`, `rule`, `hook`, `pack`).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "catalog/types.ts")]
pub struct Entry {
    pub id: String,
    /// The folder under `~/.claude`.
    #[ts(type = "\"skills\" | \"agents\" | \"rules\" | \"hooks\" | \"departments\"")]
    pub dir: String,
    pub name: String,
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title_ar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description_ar: Option<String>,
    pub state: EntryState,
    pub owner: Owner,
    /// Targets under `~/.claude`: the registry's for kun's units, what is on disk for the rest.
    pub files: Vec<String>,
    pub dependencies: Vec<String>,
    pub keywords: Vec<String>,
    /// The registry's version, or empty.
    pub version: String,
    pub can_install: bool,
    pub can_uninstall: bool,
    pub can_toggle: bool,
}

/// One disabled unit in `disabled.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Held {
    #[serde(default)]
    files: Vec<String>,
    /// Seconds since the epoch, as text.
    #[serde(default)]
    at: String,
}

/// `disabled.json`: what is moved aside, and what was removed on purpose.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Disabled {
    #[serde(default)]
    disabled: BTreeMap<String, Held>,
    #[serde(default)]
    removed: Vec<String>,
}

/// The kinds of unit the tree shows (the registry's `machine` items are not among them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Skill,
    Agent,
    Rule,
    Hook,
    Pack,
}

impl Kind {
    const ALL: [Self; 5] = [Self::Skill, Self::Agent, Self::Rule, Self::Hook, Self::Pack];

    fn prefix(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Agent => "agent",
            Self::Rule => "rule",
            Self::Hook => "hook",
            Self::Pack => "pack",
        }
    }

    fn dir(self) -> &'static str {
        match self {
            Self::Skill => "skills",
            Self::Agent => "agents",
            Self::Rule => "rules",
            Self::Hook => "hooks",
            Self::Pack => "departments",
        }
    }

    /// The path under `~/.claude` that moves as one: a folder or a single file.
    fn unit(self, name: &str) -> String {
        match self {
            Self::Skill | Self::Pack => format!("{}/{name}", self.dir()),
            Self::Agent | Self::Rule => format!("{}/{name}.md", self.dir()),
            Self::Hook => format!("{}/{name}.sh", self.dir()),
        }
    }

    /// Hooks are wired from `settings.json` (a missing file errors on every event) and packs are
    /// many files: only skills, agents and rules switch off and on.
    fn toggleable(self) -> bool {
        matches!(self, Self::Skill | Self::Agent | Self::Rule)
    }
}

/// `skill:funnel` into its kind and name.
fn parse_id(id: &str) -> Option<(Kind, &str)> {
    let (prefix, name) = id.split_once(':')?;
    if name.is_empty() {
        return None;
    }
    Kind::ALL
        .into_iter()
        .find(|k| k.prefix() == prefix)
        .map(|k| (k, name))
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid item name: use lowercase letters, digits, - and :")]
    InvalidName,
    #[error("no such item: {0}")]
    UnknownItem(String),
    #[error("registry not found at {0} — run `pnpm kun:registry` in the kun checkout")]
    NoRegistry(String),
    #[error("registry is stale for {0} — run `pnpm kun:registry`")]
    Stale(String),
    #[error("refused: {0} would leave ~/.claude")]
    Unsafe(String),
    #[error("{0} is required by installed {1}")]
    InUse(String, String),
    #[error("{0} cannot be switched off and on here")]
    NotToggleable(String),
    #[error("{0} is not from kun; this app never deletes what a person made")]
    Personal(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0} is not part of this machine's profile")]
    OutsideProfile(String),
    #[error("profile: {0}")]
    Profile(String),
    #[error("{0}")]
    Io(String),
}

impl Serialize for CatalogError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for CatalogError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

type Res<T> = std::result::Result<T, CatalogError>;

/// The three folders everything happens in. Tests point them at tempdirs.
#[derive(Debug, Clone)]
pub struct Env {
    pub kun_dir: PathBuf,
    pub claude_dir: PathBuf,
    pub kun_home: PathBuf,
}

impl Env {
    /// `KUN_DIR` (default `~/kun`), `CLAUDE_DIR` (default `~/.claude`), `KUN_HOME` (default `~/.kun`).
    #[must_use]
    pub fn from_process() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let pick = |var: &str, default: &str| {
            std::env::var_os(var).map_or_else(|| home.join(default), PathBuf::from)
        };
        Self {
            kun_dir: pick("KUN_DIR", "kun"),
            claude_dir: pick("CLAUDE_DIR", ".claude"),
            kun_home: pick("KUN_HOME", ".kun"),
        }
    }

    fn registry_path(&self) -> PathBuf {
        self.kun_dir.join("public/r/kun/index.json")
    }

    fn lock_path(&self) -> PathBuf {
        self.kun_home.join("machine/installed.json")
    }

    /// Where disabled units wait, mirroring their path under `~/.claude`.
    fn store_dir(&self) -> PathBuf {
        self.kun_home.join("machine/disabled")
    }

    /// The record of what is disabled or removed on purpose; `setup.sh` reads it.
    fn disabled_path(&self) -> PathBuf {
        self.kun_home.join("machine/disabled.json")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Item names are ids in IPC and the lockfile: `[a-z0-9-:]`, never empty.
fn validate_name(name: &str) -> Res<()> {
    let ok = !name.is_empty()
        && name.len() <= 80
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == ':');
    if ok {
        Ok(())
    } else {
        Err(CatalogError::InvalidName)
    }
}

/// A relative path made only of plain components: no `..`, no root, no prefix.
fn safe_rel(p: &str) -> Res<PathBuf> {
    let path = Path::new(p);
    let plain = path.components().count() > 0
        && path.components().all(|c| matches!(c, Component::Normal(_)));
    if plain {
        Ok(path.to_path_buf())
    } else {
        Err(CatalogError::Unsafe(p.to_string()))
    }
}

/// The destination, after resolving symlinks in its existing ancestors, must stay inside `root`.
fn ensure_within(root: &Path, dest: &Path) -> Res<()> {
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let mut probe = dest;
    let existing = loop {
        if probe.exists() {
            break probe;
        }
        match probe.parent() {
            Some(p) => probe = p,
            None => return Err(CatalogError::Unsafe(dest.display().to_string())),
        }
    };
    if existing.canonicalize()?.starts_with(&root) {
        Ok(())
    } else {
        Err(CatalogError::Unsafe(dest.display().to_string()))
    }
}

fn read_registry(env: &Env) -> Res<Vec<CatalogItem>> {
    let path = env.registry_path();
    let src = fs::read_to_string(&path)
        .map_err(|_| CatalogError::NoRegistry(path.display().to_string()))?;
    let reg: Registry =
        serde_json::from_str(&src).map_err(|e| CatalogError::Io(format!("registry: {e}")))?;
    Ok(reg.items)
}

fn read_lock(env: &Env) -> Lock {
    fs::read_to_string(env.lock_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Write `bytes` to `dest` by temp file + rename in the same directory, never in place.
fn atomic_write(dest: &Path, bytes: &[u8], mode_from: Option<&Path>) -> Res<()> {
    let dir = dest
        .parent()
        .ok_or_else(|| CatalogError::Unsafe(dest.display().to_string()))?;
    fs::create_dir_all(dir)?;
    let base = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{base}.kun-tmp.{}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        if let Some(src) = mode_from {
            fs::set_permissions(&tmp, fs::metadata(src)?.permissions())?;
        }
        fs::rename(&tmp, dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(Into::into)
}

fn write_lock(env: &Env, lock: &Lock) -> Res<()> {
    let body = serde_json::to_string_pretty(lock).map_err(|e| CatalogError::Io(e.to_string()))?;
    atomic_write(&env.lock_path(), format!("{body}\n").as_bytes(), None)
}

fn read_disabled(env: &Env) -> Disabled {
    fs::read_to_string(env.disabled_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_disabled(env: &Env, record: &Disabled) -> Res<()> {
    let body = serde_json::to_string_pretty(record).map_err(|e| CatalogError::Io(e.to_string()))?;
    atomic_write(&env.disabled_path(), format!("{body}\n").as_bytes(), None)
}

fn now_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .to_string()
}

/// Notes `id` as removed on purpose; true when that changed the record.
fn mark_removed(record: &mut Disabled, id: &str) -> bool {
    if record.removed.iter().any(|r| r == id) {
        return false;
    }
    record.removed.push(id.to_owned());
    record.removed.sort();
    true
}

/// Deletes the unit the disabled store holds for `id` and forgets it; true when it held one.
fn delete_held(env: &Env, id: &str, record: &mut Disabled) -> Res<bool> {
    if !record.disabled.contains_key(id) {
        return Ok(false);
    }
    if let Some((kind, name)) = parse_id(id) {
        let store = env.store_dir();
        let path = store.join(safe_rel(&kind.unit(name))?);
        ensure_within(&store, &path)?;
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(&path)?,
            Ok(_) => fs::remove_file(&path)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    record.disabled.remove(id);
    Ok(true)
}

/// A fresh install replaces whatever the store held and ends any "removed on purpose".
fn settle_store_after_install(env: &Env, id: &str) -> Res<()> {
    let mut record = read_disabled(env);
    let held = delete_held(env, id, &mut record)?;
    let was_removed = record.removed.iter().any(|r| r == id);
    record.removed.retain(|r| r != id);
    if held || was_removed {
        write_disabled(env, &record)?;
    }
    Ok(())
}

fn status_of(env: &Env, item: &CatalogItem, lock: &Lock) -> (ItemStatus, Option<String>) {
    let Some(entry) = lock.items.get(&item.id) else {
        return (ItemStatus::Available, None);
    };
    let current = item.files.iter().all(|f| {
        safe_rel(&f.target)
            .ok()
            .and_then(|t| fs::read(env.claude_dir.join(t)).ok())
            .is_some_and(|b| sha256_hex(&b) == f.sha256)
    });
    let status = if current {
        ItemStatus::Installed
    } else {
        ItemStatus::Outdated
    };
    (status, Some(entry.version.clone()))
}

/// The ids a profile-limited machine may touch: its departments' packs and everything they need.
/// `None` means the profile sees the whole catalog.
///
/// # Errors
/// When the registry is missing or unreadable.
pub fn allowed_ids(env: &Env, profile: &MachineProfile) -> Res<Option<BTreeSet<String>>> {
    if profile.access.catalog == CatalogAccess::All {
        return Ok(None);
    }
    let items = read_registry(env)?;
    let mut allowed = BTreeSet::new();
    for dept in &profile.departments {
        let id = format!("pack:{}", dept.name);
        if items.iter().any(|i| i.id == id) {
            allowed.extend(plan(&items, &id)?);
        }
    }
    for id in &profile.hidden {
        allowed.remove(id);
    }
    Ok(Some(allowed))
}

/// Every registry item with its status here.
///
/// # Errors
/// When the registry is missing or unreadable.
pub fn list_items(env: &Env) -> Res<Vec<CatalogItem>> {
    list_items_scoped(env, None)
}

/// [`list_items`], limited to `allowed` when it is `Some`.
///
/// # Errors
/// When the registry is missing or unreadable.
pub fn list_items_scoped(env: &Env, allowed: Option<&BTreeSet<String>>) -> Res<Vec<CatalogItem>> {
    let lock = read_lock(env);
    let mut items = read_registry(env)?;
    if let Some(allowed) = allowed {
        items.retain(|i| allowed.contains(&i.id));
    }
    for item in &mut items {
        let (status, installed_version) = status_of(env, item, &lock);
        item.status = status;
        item.installed_version = installed_version;
    }
    Ok(items)
}

/// `id` and everything it needs, dependencies first.
fn plan(items: &[CatalogItem], id: &str) -> Res<Vec<String>> {
    fn visit(
        items: &[CatalogItem],
        id: &str,
        seen: &mut Vec<String>,
        order: &mut Vec<String>,
    ) -> Res<()> {
        if seen.iter().any(|s| s == id) {
            return Ok(()); // already planned, or a cycle: either way do not recurse again
        }
        seen.push(id.to_string());
        let item = items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| CatalogError::UnknownItem(id.to_string()))?;
        for dep in &item.dependencies {
            visit(items, dep, seen, order)?;
        }
        order.push(id.to_string());
        Ok(())
    }
    let mut order = Vec::new();
    visit(items, id, &mut Vec::new(), &mut order)?;
    Ok(order)
}

/// Is `target` still claimed by an installed item other than `except`?
fn owned_elsewhere(lock: &Lock, except: &str, target: &str) -> bool {
    lock.items
        .iter()
        .any(|(id, e)| id != except && e.files.iter().any(|f| f == target))
}

fn install_one(env: &Env, item: &CatalogItem, lock: &mut Lock) -> Res<()> {
    // Resolve and check everything before writing anything.
    let mut staged: Vec<(PathBuf, Vec<u8>, PathBuf, String)> = Vec::new();
    for f in &item.files {
        let src = env.kun_dir.join(safe_rel(&f.path)?);
        let dest = env.claude_dir.join(safe_rel(&f.target)?);
        ensure_within(&env.claude_dir, &dest)?;
        let bytes = fs::read(&src).map_err(|_| CatalogError::Stale(f.path.clone()))?;
        if sha256_hex(&bytes) != f.sha256 {
            return Err(CatalogError::Stale(f.path.clone()));
        }
        staged.push((src, bytes, dest, f.target.clone()));
    }
    for (src, bytes, dest, _) in &staged {
        atomic_write(dest, bytes, Some(src))?;
    }
    // Files an older version put down that this version no longer has.
    let new_targets: Vec<String> = staged.iter().map(|s| s.3.clone()).collect();
    if let Some(old) = lock.items.get(&item.id).cloned() {
        for t in old.files.iter().filter(|t| !new_targets.contains(t)) {
            if !owned_elsewhere(lock, &item.id, t) {
                remove_target(env, t);
            }
        }
    }
    lock.items.insert(
        item.id.clone(),
        Installed {
            version: item.version.clone(),
            files: new_targets,
            dependencies: item.dependencies.clone(),
        },
    );
    write_lock(env, lock)
}

/// Remove one recorded file, then any now-empty directories above it (never `~/.claude` itself).
fn remove_target(env: &Env, target: &str) {
    let Ok(rel) = safe_rel(target) else { return };
    let path = env.claude_dir.join(rel);
    if fs::remove_file(&path).is_err() {
        return;
    }
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == env.claude_dir || fs::remove_dir(d).is_err() {
            break; // remove_dir only succeeds on an empty directory
        }
        dir = d.parent();
    }
}

/// Install `name` and its dependencies. Returns the ids written, dependencies first.
///
/// # Errors
/// On an invalid or unknown name, a stale registry, a path that would leave `~/.claude`, or I/O.
pub fn install_item(env: &Env, name: &str) -> Res<Vec<String>> {
    install_item_scoped(env, name, None)
}

/// [`install_item`], refusing `name` when `allowed` is `Some` and does not list it. Dependencies
/// of an allowed item are allowed by construction ([`allowed_ids`]).
///
/// # Errors
/// As [`install_item`], plus [`CatalogError::OutsideProfile`].
pub fn install_item_scoped(
    env: &Env,
    name: &str,
    allowed: Option<&BTreeSet<String>>,
) -> Res<Vec<String>> {
    validate_name(name)?;
    if allowed.is_some_and(|a| !a.contains(name)) {
        return Err(CatalogError::OutsideProfile(name.to_string()));
    }
    let items = list_items(env)?;
    let order = plan(&items, name)?;
    let mut lock = read_lock(env);
    let mut done = Vec::new();
    for id in order {
        // A dependency the profile hides (a pack's Mac-only lane) is skipped, never installed.
        if allowed.is_some_and(|a| !a.contains(&id)) {
            continue;
        }
        let item = items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| CatalogError::UnknownItem(id.clone()))?;
        // The asked-for item is always (re)written; dependencies only when not current.
        if id != name && item.status == ItemStatus::Installed {
            continue;
        }
        install_one(env, item, &mut lock)?;
        settle_store_after_install(env, &id)?;
        done.push(id);
    }
    Ok(done)
}

/// Remove an item kun put on disk: the files the lockfile recorded for it, or, when the engine's
/// own installer shipped it, the files the registry lists; the held copy when it is disabled.
/// Either way it is noted as removed on purpose.
///
/// # Errors
/// On an invalid or unknown name, when another installed item depends on it, or
/// [`CatalogError::Personal`] for what a person made: the app never deletes that.
pub fn uninstall_item(env: &Env, name: &str) -> Res<()> {
    uninstall_item_scoped(env, name, None)
}

/// [`uninstall_item`], refusing `name` when `allowed` is `Some` and does not list it.
///
/// # Errors
/// As [`uninstall_item`], plus [`CatalogError::OutsideProfile`].
pub fn uninstall_item_scoped(env: &Env, name: &str, allowed: Option<&BTreeSet<String>>) -> Res<()> {
    validate_name(name)?;
    if allowed.is_some_and(|a| !a.contains(name)) {
        return Err(CatalogError::OutsideProfile(name.to_string()));
    }
    let mut lock = read_lock(env);
    let mut record = read_disabled(env);
    let held = record.disabled.contains_key(name);
    if let Some(entry) = lock.items.get(name).cloned() {
        check_in_use(&lock, name)?;
        lock.items.remove(name);
        for t in &entry.files {
            if !owned_elsewhere(&lock, name, t) {
                remove_target(env, t);
            }
        }
        write_lock(env, &lock)?;
    } else {
        // Not in this app's lockfile: the engine's own installer may have shipped it.
        let registry = registry_or_empty(env)?;
        let Some(item) = registry.iter().find(|i| i.id == name) else {
            let made_here = held
                || parse_id(name).is_some_and(|(kind, n)| {
                    safe_rel(&kind.unit(n))
                        .is_ok_and(|rel| fs::symlink_metadata(env.claude_dir.join(rel)).is_ok())
                });
            return Err(if made_here {
                CatalogError::Personal(name.to_owned())
            } else {
                CatalogError::UnknownItem(name.to_owned())
            });
        };
        if parse_id(name).is_some_and(|(kind, _)| kind == Kind::Hook) {
            return Err(CatalogError::Conflict(format!(
                "{name} is wired from the engine's settings; removing the file would break every event"
            )));
        }
        let on_disk = item.files.iter().any(|f| {
            safe_rel(&f.target).is_ok_and(|t| fs::symlink_metadata(env.claude_dir.join(t)).is_ok())
        });
        if !on_disk && !held {
            return Err(CatalogError::UnknownItem(name.to_owned()));
        }
        check_in_use(&lock, name)?;
        for f in &item.files {
            if !owned_elsewhere(&lock, name, &f.target) {
                remove_target(env, &f.target);
            }
        }
    }
    delete_held(env, name, &mut record)?;
    mark_removed(&mut record, name);
    write_disabled(env, &record)
}

/// Refuses to remove `name` while another installed item needs it.
fn check_in_use(lock: &Lock, name: &str) -> Res<()> {
    match lock
        .items
        .iter()
        .find(|(id, e)| id.as_str() != name && e.dependencies.iter().any(|d| d == name))
    {
        Some((other, _)) => Err(CatalogError::InUse(name.to_owned(), other.clone())),
        None => Ok(()),
    }
}

/// A registry file's state on disk: how many of an item's files exist and how many match.
fn file_state(env: &Env, item: &CatalogItem) -> EntryState {
    let (mut present, mut matching) = (0, 0);
    for f in &item.files {
        let bytes = safe_rel(&f.target)
            .ok()
            .and_then(|t| fs::read(env.claude_dir.join(t)).ok());
        if let Some(bytes) = bytes {
            present += 1;
            if sha256_hex(&bytes) == f.sha256 {
                matching += 1;
            }
        }
    }
    if present == 0 {
        EntryState::Available
    } else if matching == item.files.len() {
        EntryState::Installed
    } else {
        EntryState::Outdated
    }
}

/// The registry, or nothing when kun has not built one here: the tree is about what is on disk.
fn registry_or_empty(env: &Env) -> Res<Vec<CatalogItem>> {
    match read_registry(env) {
        Err(CatalogError::NoRegistry(_)) => Ok(Vec::new()),
        other => other,
    }
}

/// A unit found on disk.
struct Found {
    kind: Kind,
    name: String,
    files: Vec<String>,
}

/// The most files listed for one personal folder.
const MAX_UNIT_FILES: usize = 1000;

/// The plain files under `dir` as paths relative to `~/.claude`; dot-files and symlinks skipped.
fn walk_files(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.filter_map(std::result::Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        if out.len() >= MAX_UNIT_FILES {
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if name.starts_with('.') || kind.is_symlink() {
            continue;
        }
        let path = format!("{rel}/{name}");
        if kind.is_dir() {
            walk_files(&entry.path(), &path, out);
        } else if kind.is_file() {
            out.push(path);
        }
    }
}

/// Scans `~/.claude` for every unit of every kind, by id.
fn scan_disk(env: &Env) -> BTreeMap<String, Found> {
    let mut found = BTreeMap::new();
    let Ok(root) = env.claude_dir.canonicalize() else {
        return found;
    };
    for kind in Kind::ALL {
        let Ok(read) = fs::read_dir(env.claude_dir.join(kind.dir())) else {
            continue;
        };
        for entry in read.filter_map(std::result::Result::ok) {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            // A link that leads out of `~/.claude` is not ours to list.
            let linked = entry.file_type().is_ok_and(|t| t.is_symlink());
            if linked && !path.canonicalize().is_ok_and(|p| p.starts_with(&root)) {
                continue;
            }
            let name = match kind {
                Kind::Skill => {
                    (path.is_dir() && path.join("SKILL.md").is_file()).then(|| file_name.clone())
                }
                Kind::Pack => path.is_dir().then(|| file_name.clone()),
                Kind::Agent | Kind::Rule => path
                    .is_file()
                    .then(|| file_name.strip_suffix(".md").map(str::to_owned))
                    .flatten(),
                Kind::Hook => path
                    .is_file()
                    .then(|| file_name.strip_suffix(".sh").map(str::to_owned))
                    .flatten(),
            };
            let Some(name) = name.filter(|n| !n.is_empty()) else {
                continue;
            };
            let unit = kind.unit(&name);
            let files = if matches!(kind, Kind::Skill | Kind::Pack) {
                let mut files = Vec::new();
                walk_files(&path, &unit, &mut files);
                files
            } else {
                vec![unit]
            };
            found.insert(
                format!("{}:{name}", kind.prefix()),
                Found { kind, name, files },
            );
        }
    }
    found
}

/// Builds one entry from its parts; the three `can_*` follow from state, owner and kind.
fn make_entry(
    kind: Kind,
    name: &str,
    state: EntryState,
    item: Option<&CatalogItem>,
    files: Vec<String>,
) -> Entry {
    let kun = item.is_some();
    let present = matches!(
        state,
        EntryState::Installed | EntryState::Outdated | EntryState::Personal | EntryState::Disabled
    );
    Entry {
        id: format!("{}:{name}", kind.prefix()),
        dir: kind.dir().to_owned(),
        name: name.to_owned(),
        title: item.map_or_else(|| name.to_owned(), |i| i.title.clone()),
        description: item.map(|i| i.description.clone()).unwrap_or_default(),
        title_ar: item.and_then(|i| i.title_ar.clone()),
        description_ar: item.and_then(|i| i.description_ar.clone()),
        state,
        owner: if kun { Owner::Kun } else { Owner::You },
        files,
        dependencies: item.map(|i| i.dependencies.clone()).unwrap_or_default(),
        keywords: item.map(|i| i.keywords.clone()).unwrap_or_default(),
        version: item.map(|i| i.version.clone()).unwrap_or_default(),
        can_install: kun && matches!(state, EntryState::Available | EntryState::Outdated),
        can_uninstall: kun
            && matches!(
                state,
                EntryState::Installed | EntryState::Outdated | EntryState::Disabled
            ),
        can_toggle: kind.toggleable() && present && validate_name(name).is_ok(),
    }
}

/// The real `~/.claude` tree merged with the registry.
///
/// # Errors
/// When the registry exists but cannot be read.
pub fn tree_entries(env: &Env) -> Res<Vec<Entry>> {
    tree_entries_scoped(env, None)
}

/// [`tree_entries`], with kun's units limited to `allowed` when it is `Some`; units the person
/// made are always listed.
///
/// # Errors
/// When the registry exists but cannot be read.
pub fn tree_entries_scoped(env: &Env, allowed: Option<&BTreeSet<String>>) -> Res<Vec<Entry>> {
    let registry = registry_or_empty(env)?;
    let kun_ids: BTreeSet<&str> = registry
        .iter()
        .filter(|i| parse_id(&i.id).is_some())
        .map(|i| i.id.as_str())
        .collect();
    let disk = scan_disk(env);
    let record = read_disabled(env);
    let lock = read_lock(env);
    let mut out = Vec::new();

    for item in &registry {
        let Some((kind, name)) = parse_id(&item.id) else {
            continue;
        };
        if allowed.is_some_and(|a| !a.contains(&item.id)) {
            continue;
        }
        let state = if record.disabled.contains_key(&item.id) {
            EntryState::Disabled
        } else {
            file_state(env, item)
        };
        let files = item.files.iter().map(|f| f.target.clone()).collect();
        let mut entry = make_entry(kind, name, state, Some(item), files);
        // A hook the engine's own installer shipped is wired from `settings.json`: without the
        // file every event would error. Only a hook this app installed can be removed here.
        if kind == Kind::Hook && !lock.items.contains_key(&item.id) {
            entry.can_uninstall = false;
        }
        out.push(entry);
    }
    for (id, found) in &disk {
        if kun_ids.contains(id.as_str()) || record.disabled.contains_key(id) {
            continue;
        }
        out.push(make_entry(
            found.kind,
            &found.name,
            EntryState::Personal,
            None,
            found.files.clone(),
        ));
    }
    for (id, held) in &record.disabled {
        if kun_ids.contains(id.as_str()) {
            continue;
        }
        if let Some((kind, name)) = parse_id(id) {
            out.push(make_entry(
                kind,
                name,
                EntryState::Disabled,
                None,
                held.files.clone(),
            ));
        }
    }
    out.sort_by(|a, b| (&a.dir, &a.name).cmp(&(&b.dir, &b.name)));
    Ok(out)
}

/// Refuses a registry-backed `id` the profile does not list; a unit the person made is never
/// outside the profile.
fn check_scope(env: &Env, id: &str, allowed: Option<&BTreeSet<String>>) -> Res<()> {
    let Some(allowed) = allowed else {
        return Ok(());
    };
    if allowed.contains(id) || !registry_or_empty(env)?.iter().any(|i| i.id == id) {
        return Ok(());
    }
    Err(CatalogError::OutsideProfile(id.to_owned()))
}

/// What `disable` and `enable` share: a valid id of a kind that may be toggled, inside the
/// profile, and its path under `~/.claude`.
fn toggle_target(
    env: &Env,
    id: &str,
    allowed: Option<&BTreeSet<String>>,
) -> Res<(String, PathBuf)> {
    validate_name(id)?;
    let (kind, name) = parse_id(id).ok_or_else(|| CatalogError::UnknownItem(id.to_owned()))?;
    if !kind.toggleable() || validate_name(name).is_err() {
        return Err(CatalogError::NotToggleable(id.to_owned()));
    }
    check_scope(env, id, allowed)?;
    let unit = kind.unit(name);
    let rel = safe_rel(&unit)?;
    Ok((unit, rel))
}

/// Moves a skill, agent or rule out of `~/.claude` into the disabled store, keeping it.
///
/// # Errors
/// [`CatalogError::NotToggleable`] for hooks, packs and bad names; `Unsafe` for a symlinked unit;
/// `UnknownItem` when it is not on disk; `Conflict` when the store already holds it; or I/O.
pub fn disable_item(env: &Env, id: &str) -> Res<()> {
    disable_item_scoped(env, id, None)
}

/// [`disable_item`], refusing a registry-backed `id` the profile does not list.
///
/// # Errors
/// As [`disable_item`], plus [`CatalogError::OutsideProfile`].
pub fn disable_item_scoped(env: &Env, id: &str, allowed: Option<&BTreeSet<String>>) -> Res<()> {
    let (unit, rel) = toggle_target(env, id, allowed)?;
    let mut record = read_disabled(env);
    if record.disabled.contains_key(id) {
        return Err(CatalogError::Conflict(format!("{id} is already disabled")));
    }
    let src = env.claude_dir.join(&rel);
    match fs::symlink_metadata(&src) {
        Err(_) => return Err(CatalogError::UnknownItem(id.to_owned())),
        Ok(meta) if meta.file_type().is_symlink() => return Err(CatalogError::Unsafe(unit)),
        Ok(_) => {}
    }
    ensure_within(&env.claude_dir, &src)?;
    let store = env.store_dir();
    let dest = store.join(&rel);
    ensure_within(&store, &dest)?;
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(CatalogError::Conflict(format!(
            "the disabled store already holds {unit}"
        )));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&src, &dest)?;
    record.disabled.insert(
        id.to_owned(),
        Held {
            files: vec![unit],
            at: now_secs(),
        },
    );
    if let Err(e) = write_disabled(env, &record) {
        let _ = fs::rename(&dest, &src); // keep the record and the disk telling the same story
        return Err(e);
    }
    Ok(())
}

/// Moves a disabled unit back into `~/.claude`.
///
/// # Errors
/// [`CatalogError::NotToggleable`] for hooks, packs and bad names; `UnknownItem` when the store
/// holds nothing for it; `Conflict` when something already sits at the destination; or I/O.
pub fn enable_item(env: &Env, id: &str) -> Res<()> {
    enable_item_scoped(env, id, None)
}

/// [`enable_item`], refusing a registry-backed `id` the profile does not list.
///
/// # Errors
/// As [`enable_item`], plus [`CatalogError::OutsideProfile`].
pub fn enable_item_scoped(env: &Env, id: &str, allowed: Option<&BTreeSet<String>>) -> Res<()> {
    let (unit, rel) = toggle_target(env, id, allowed)?;
    let mut record = read_disabled(env);
    if !record.disabled.contains_key(id) {
        return Err(CatalogError::UnknownItem(id.to_owned()));
    }
    let store = env.store_dir();
    let src = store.join(&rel);
    ensure_within(&store, &src)?;
    match fs::symlink_metadata(&src) {
        Err(_) => return Err(CatalogError::UnknownItem(id.to_owned())),
        Ok(meta) if meta.file_type().is_symlink() => return Err(CatalogError::Unsafe(unit)),
        Ok(_) => {}
    }
    let dest = env.claude_dir.join(&rel);
    ensure_within(&env.claude_dir, &dest)?;
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(CatalogError::Conflict(format!(
            "{unit} already exists in ~/.claude"
        )));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&src, &dest)?;
    record.disabled.remove(id);
    if let Err(e) = write_disabled(env, &record) {
        let _ = fs::rename(&dest, &src);
        return Err(e);
    }
    Ok(())
}

/// Installs and uninstalls rewrite the lockfile: one at a time.
static WRITE: Mutex<()> = Mutex::new(());

async fn blocking<T: Send + 'static>(f: impl FnOnce(&Env) -> Res<T> + Send + 'static) -> Res<T> {
    tokio::task::spawn_blocking(move || {
        let _guard = WRITE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&Env::from_process())
    })
    .await
    .map_err(|e| CatalogError::Io(e.to_string()))?
}

/// This machine's allowed ids from its profile; `None` when the catalog is unrestricted.
fn scope(env: &Env) -> Res<Option<BTreeSet<String>>> {
    let resolved = crate::profile::current().map_err(|e| CatalogError::Profile(e.to_string()))?;
    allowed_ids(env, &resolved.profile)
}

/// `list` — the registry with each item's status on this computer, limited to the profile.
#[tauri::command]
async fn list() -> Res<Vec<CatalogItem>> {
    blocking(|env| list_items_scoped(env, scope(env)?.as_ref())).await
}

/// `install` — an item and its dependencies; returns the ids written. Refuses anything
/// outside the profile.
#[tauri::command]
async fn install(name: String) -> Res<Vec<String>> {
    blocking(move |env| install_item_scoped(env, &name, scope(env)?.as_ref())).await
}

/// `uninstall` — only the files this catalog installed for the item.
#[tauri::command]
async fn uninstall(name: String) -> Res<()> {
    blocking(move |env| uninstall_item_scoped(env, &name, scope(env)?.as_ref())).await
}

/// `tree` — the real `~/.claude` (skills, agents, rules, hooks, packs) merged with the registry,
/// limited to the profile for kun's units.
#[tauri::command]
async fn tree() -> Res<Vec<Entry>> {
    blocking(|env| tree_entries_scoped(env, scope(env)?.as_ref())).await
}

/// `disable` — moves a skill, agent or rule aside without deleting it.
#[tauri::command]
async fn disable(id: String) -> Res<()> {
    blocking(move |env| disable_item_scoped(env, &id, scope(env)?.as_ref())).await
}

/// `enable` — moves a disabled skill, agent or rule back.
#[tauri::command]
async fn enable(id: String) -> Res<()> {
    blocking(move |env| enable_item_scoped(env, &id, scope(env)?.as_ref())).await
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("catalog")
        .invoke_handler(tauri::generate_handler![
            list, install, uninstall, tree, disable, enable
        ])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
