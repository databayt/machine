//! production — the video line's state for the strip above the chat composer.
//!
//! Three read-only commands, all with no shell:
//! - `production_list(repo)` runs `node <produce.mjs> list --json --repo <repo>`;
//! - `production_status(id)` runs `produce.mjs status <id> --json` and reads the project's
//!   `shot-list.json` and `asset-manifest.json` beside it, handing the raw JSON to the UI (the pure
//!   parsers in `pure.ts` own the shape, so a field kun adds never breaks the strip);
//! - `production_brand(identity)` reads `<kun>/content/media/brand-kit.json` → `production.<identity>`
//!   once and caches it.
//!
//! `production_exist(paths)` marks missing assets. `production_thumb(path)` returns a small image as a data URL for the Scenes tab: the media
//! library uses CDN links, and the asset protocol would open a webview scope over the whole disk, so
//! local thumbnails go through this one checked command instead (image files under the home folder,
//! 1.5 MB at most).

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{LazyLock, Mutex, PoisonError},
    time::Duration,
};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::process::Command;

use crate::pack::{self, health};

/// The script, relative to a skills folder.
const SCRIPT: &str = "produce/scripts/produce.mjs";
const LIST_TIMEOUT: Duration = Duration::from_secs(10);
const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_TIMEOUT: Duration = Duration::from_secs(20);
/// The biggest spec (one `kun-spec` block) and plan (all 14 stages) the app writes to disk.
const MAX_SPEC_BYTES: usize = 64 * 1024;
const MAX_PLAN_BYTES: usize = 256 * 1024;
/// The biggest thumbnail sent over IPC.
const MAX_THUMB_BYTES: u64 = 1_500_000;

#[derive(Debug, thiserror::Error)]
pub enum ProductionError {
    #[error("{0}")]
    Bad(String),
    #[error("{0}")]
    Run(String),
}

impl Serialize for ProductionError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub use crate::pack::station::{STAGE_KEYS, valid_film_id};

/// The arguments of `list`, after the script path.
///
/// # Errors
/// On a repo that is not a plain name.
pub fn list_args(repo: &str) -> Result<Vec<String>, ProductionError> {
    let mut a = vec!["list".to_owned(), "--json".to_owned()];
    if !repo.is_empty() {
        if !pack::valid_name(repo) {
            return Err(ProductionError::Bad("repo name is invalid".to_owned()));
        }
        a.extend(["--repo".to_owned(), repo.to_owned()]);
    }
    Ok(a)
}

/// The arguments of `status`, after the script path.
///
/// # Errors
/// On an id that is not a film id.
pub fn status_args(id: &str) -> Result<Vec<String>, ProductionError> {
    if !valid_film_id(id) {
        return Err(ProductionError::Bad("film id is invalid".to_owned()));
    }
    Ok(vec!["status".to_owned(), id.to_owned(), "--json".to_owned()])
}

/// The JSON value in a script's stdout; it may follow log noise, so the first `{` or `[` is tried too.
#[must_use]
pub fn json_of(stdout: &str) -> Option<Value> {
    serde_json::from_str::<Value>(stdout.trim()).ok().or_else(|| {
        let start = stdout.find(['{', '['])?;
        serde_json::from_str::<Value>(stdout[start..].trim()).ok()
    })
}

/// The produce script: the user's skills folder first, else kun's checkout.
fn script_path(home: &Path, kun: &Path) -> Option<PathBuf> {
    [home.join(".claude/skills"), kun.join(".claude/skills")]
        .into_iter()
        .map(|d| d.join(SCRIPT))
        .find(|p| p.is_file())
}

/// The produce script with kun's checkout first: a freshly updated kun can be ahead of the installed copy.
fn script_path_kun_first(home: &Path, kun: &Path) -> Option<PathBuf> {
    [kun.join(".claude/skills"), home.join(".claude/skills")]
        .into_iter()
        .map(|d| d.join(SCRIPT))
        .find(|p| p.is_file())
}

async fn run_produce(extra: Vec<String>, timeout: Duration) -> Result<String, ProductionError> {
    run_produce_with(extra, timeout, false).await
}

async fn run_produce_with(extra: Vec<String>, timeout: Duration, kun_first: bool) -> Result<String, ProductionError> {
    let kun = pack::kun_dir().map_err(|e| ProductionError::Run(e.to_string()))?;
    let home = dirs::home_dir().ok_or_else(|| ProductionError::Run("could not find the home folder".to_owned()))?;
    let found = if kun_first { script_path_kun_first(&home, &kun) } else { script_path(&home, &kun) };
    let script = found.ok_or_else(|| ProductionError::Run("the produce skill is not installed".to_owned()))?;
    let mut argv = vec!["node".to_owned(), script.display().to_string()];
    argv.extend(extra);
    let checked = health::safe_argv(&argv, &home, &kun).map_err(ProductionError::Run)?;

    let dirs = crate::driver::search_dirs();
    let node = dirs
        .iter()
        .map(|d| d.join("node"))
        .find(|p| p.is_file())
        .ok_or_else(|| ProductionError::Run("node was not found on this computer".to_owned()))?;
    let mut cmd = Command::new(node);
    cmd.args(&checked[1..])
        .current_dir(&home)
        .env("PATH", crate::driver::run_path(&dirs))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| ProductionError::Run(format!("produce timed out after {} s", timeout.as_secs())))?
        .map_err(|e| ProductionError::Run(format!("could not run produce: {e}")))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if stdout.trim().is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no output");
        return Err(ProductionError::Run(format!("produce failed: {}", line.trim())));
    }
    Ok(stdout)
}

/// A set key: `[a-z_]{1,32}`.
fn valid_set_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 32 && k.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}

/// A set value: 1 to 300 characters, no control characters.
fn valid_set_value(v: &str) -> bool {
    let n = v.chars().count();
    (1..=300).contains(&n) && !v.chars().any(char::is_control)
}

/// The arguments of `lock`, after the script path: one stage or `--all`, never both, never neither.
///
/// # Errors
/// On a bad film id, stage, set entry, login, or a spec file together with `--all`.
pub fn lock_args(
    id: &str,
    stage: Option<&str>,
    all: bool,
    set: &BTreeMap<String, String>,
    spec_file: Option<&Path>,
    login: &str,
) -> Result<Vec<String>, ProductionError> {
    let bad = |m: &str| ProductionError::Bad(m.to_owned());
    if !valid_film_id(id) {
        return Err(bad("film id is invalid"));
    }
    if !crate::profile::valid_login(login) {
        return Err(bad("the login is invalid"));
    }
    let mut a = vec!["lock".to_owned(), id.to_owned()];
    match (all, stage) {
        (true, None) => a.push("--all".to_owned()),
        (false, Some(s)) if STAGE_KEYS.contains(&s) => a.push(s.to_owned()),
        (false, Some(s)) => return Err(ProductionError::Bad(format!("unknown stage `{s}`"))),
        _ => return Err(bad("lock one stage or all of them, not both and not neither")),
    }
    for (k, v) in set {
        if !valid_set_key(k) || !valid_set_value(v) {
            return Err(ProductionError::Bad(format!("setting `{k}` is invalid")));
        }
        a.extend(["--set".to_owned(), format!("{k}={v}")]);
    }
    if let Some(f) = spec_file {
        if all {
            return Err(bad("a spec file goes with one stage, not with all"));
        }
        a.extend(["--spec".to_owned(), f.display().to_string()]);
    }
    a.extend(["--by".to_owned(), login.to_owned(), "--no-sync".to_owned(), "--json".to_owned()]);
    Ok(a)
}

/// A spec text: a JSON object of at most 64 KB whose `stage` is the one being locked.
///
/// # Errors
/// When the text is too big, not an object, or names another stage.
pub fn check_spec(text: &str, stage: &str) -> Result<Value, ProductionError> {
    if text.len() > MAX_SPEC_BYTES {
        return Err(ProductionError::Bad("the spec is too big".to_owned()));
    }
    let v: Value = serde_json::from_str(text).map_err(|_| ProductionError::Bad("the spec is not JSON".to_owned()))?;
    if !v.is_object() {
        return Err(ProductionError::Bad("the spec is not a JSON object".to_owned()));
    }
    if v.get("stage").and_then(Value::as_str) != Some(stage) {
        return Err(ProductionError::Bad("the spec names another stage".to_owned()));
    }
    Ok(v)
}

/// A plan text: a JSON object of at most 256 KB.
///
/// # Errors
/// When the text is too big or not an object.
pub fn check_plan(text: &str) -> Result<Value, ProductionError> {
    if text.len() > MAX_PLAN_BYTES {
        return Err(ProductionError::Bad("the plan is too big".to_owned()));
    }
    let v: Value = serde_json::from_str(text).map_err(|_| ProductionError::Bad("the plan is not JSON".to_owned()))?;
    if !v.is_object() {
        return Err(ProductionError::Bad("the plan is not a JSON object".to_owned()));
    }
    Ok(v)
}

/// Where a saved plan goes: `<dir>/<ms>.json`.
#[must_use]
pub fn plan_path(dir: &Path, ms: u128) -> PathBuf {
    dir.join(format!("{ms}.json"))
}

/// Where a spec goes before it is handed to `lock --spec`: `<dir>/<id>-<stage>-<ms>.json`.
#[must_use]
pub fn spec_path(dir: &Path, id: &str, stage: &str, ms: u128) -> PathBuf {
    dir.join(format!("{id}-{stage}-{ms}.json"))
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// Writes `text` under `<machine dir>/video/<sub>/`, creating the folder.
fn write_video_file(sub: &str, name: impl FnOnce(&Path) -> PathBuf, text: &str) -> Result<PathBuf, ProductionError> {
    let dir = pack::machine_dir().map_err(|e| ProductionError::Run(e.to_string()))?.join("video").join(sub);
    fs::create_dir_all(&dir).map_err(|e| ProductionError::Run(format!("could not create {}: {e}", dir.display())))?;
    let path = name(&dir);
    fs::write(&path, text).map_err(|e| ProductionError::Run(format!("could not write {}: {e}", path.display())))?;
    Ok(path)
}

/// `production_lock` — records a stage spec (or every stage) in the film's ledger; no model.
#[tauri::command]
async fn production_lock(
    id: String,
    stage: Option<String>,
    all: bool,
    set: Option<BTreeMap<String, String>>,
    spec: Option<String>,
) -> Result<Value, ProductionError> {
    let login = crate::settings::load().map(|s| s.login).unwrap_or_default();
    let set = set.unwrap_or_default();
    // Validate everything before anything is written.
    lock_args(&id, stage.as_deref(), all, &set, None, &login)?;
    let spec_file = match (&spec, stage.as_deref()) {
        (Some(text), Some(st)) => {
            let v = check_spec(text, st)?;
            let pretty = serde_json::to_string_pretty(&v).map_err(|e| ProductionError::Bad(e.to_string()))?;
            Some(write_video_file("specs", |d| spec_path(d, &id, st, now_ms()), &pretty)?)
        }
        (Some(_), None) => return Err(ProductionError::Bad("a spec goes with one stage, not with all".to_owned())),
        _ => None,
    };
    let args = lock_args(&id, stage.as_deref(), all, &set, spec_file.as_deref(), &login)?;
    let out = run_produce_with(args, LOCK_TIMEOUT, true).await?;
    json_of(&out).filter(Value::is_object).ok_or_else(|| ProductionError::Run("produce printed something that is not JSON".to_owned()))
}

/// `production_save_plan` — an accepted plan (JSON text) to `<machine dir>/video/plans/<ms>.json`; returns the path.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn production_save_plan(plan: String) -> Result<String, ProductionError> {
    let v = check_plan(&plan)?;
    let pretty = serde_json::to_string_pretty(&v).map_err(|e| ProductionError::Bad(e.to_string()))?;
    let path = write_video_file("plans", |d| plan_path(d, now_ms()), &pretty)?;
    Ok(path.display().to_string())
}

/// Where the films live: `PRODUCE_HOME`, else `~/media/_projects`.
fn projects_root(home: &Path) -> PathBuf {
    match std::env::var_os("PRODUCE_HOME") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => home.join("media/_projects"),
    }
}

/// Reads a JSON file when it exists and sits under `home`.
fn read_json_under(path: &Path, home: &Path) -> Option<Value> {
    let real = path.canonicalize().ok()?;
    if !real.starts_with(home.canonicalize().ok()?) || !real.is_file() {
        return None;
    }
    serde_json::from_str(&fs::read_to_string(real).ok()?).ok()
}

/// A path the status JSON names for one of the two files (several spellings are tried), else the default under the film's folder.
fn file_of(status: Option<&Value>, keys: &[&str], default: &Path, home: &Path) -> PathBuf {
    let named = status.and_then(|s| {
        keys.iter().find_map(|k| {
            let raw = s.get(*k).or_else(|| s.get("paths").and_then(|p| p.get(*k)))?.as_str()?;
            Some(raw.strip_prefix("~/").map_or_else(|| PathBuf::from(raw), |r| home.join(r)))
        })
    });
    named.filter(|p| p.is_absolute()).unwrap_or_else(|| default.to_path_buf())
}

/// The status of one film plus the two files the Scenes tab reads. Pure over its inputs.
#[must_use]
pub fn status_in(stdout: &str, id: &str, home: &Path, root: &Path) -> Value {
    let status = json_of(stdout).filter(Value::is_object);
    let dir = root.join(id);
    let shot = file_of(status.as_ref(), &["shotList", "shot_list", "shotListPath"], &dir.join("04-storyboard/shot-list.json"), home);
    let manifest = file_of(status.as_ref(), &["assetManifest", "asset_manifest", "assetManifestPath"], &dir.join("05-assets/asset-manifest.json"), home);
    json!({
        "status": status,
        "shotList": read_json_under(&shot, home),
        "assetManifest": read_json_under(&manifest, home),
    })
}

/// `production_list` — the films of a repo, newest first as `produce list --json` orders them.
#[tauri::command]
async fn production_list(repo: Option<String>) -> Result<Value, ProductionError> {
    let args = list_args(repo.as_deref().unwrap_or_default())?;
    let out = run_produce(args, LIST_TIMEOUT).await?;
    json_of(&out).filter(Value::is_array).ok_or_else(|| ProductionError::Run("produce printed something that is not a list".to_owned()))
}

/// `production_status` — `{ status, shotList, assetManifest }`, each null when missing.
#[tauri::command]
async fn production_status(id: String) -> Result<Value, ProductionError> {
    let args = status_args(&id)?;
    let out = run_produce(args, STATUS_TIMEOUT).await?;
    let home = dirs::home_dir().ok_or_else(|| ProductionError::Run("could not find the home folder".to_owned()))?;
    let value = status_in(&out, &id, &home, &projects_root(&home));
    if value["status"].is_null() {
        return Err(ProductionError::Run("produce printed something that is not a status".to_owned()));
    }
    Ok(value)
}

static BRANDS: LazyLock<Mutex<HashMap<String, Value>>> = LazyLock::new(Mutex::default);

/// The brand's on-screen kit from `brand-kit.json`: `{ identity, logo, logoData, fonts, palette, voice }`.
/// The logo is looked up under kun's checkout first (where the marks live), then under the brand's repo.
///
/// # Errors
/// On a bad identity, or a kit that does not name it.
pub fn brand_in(kun: &Path, home: &Path, identity: &str) -> Result<Value, ProductionError> {
    if !pack::valid_name(identity) {
        return Err(ProductionError::Bad("identity is invalid".to_owned()));
    }
    let raw = fs::read_to_string(kun.join("content/media/brand-kit.json")).map_err(|e| ProductionError::Run(format!("brand-kit.json: {e}")))?;
    let kit: Value = serde_json::from_str(&raw).map_err(|e| ProductionError::Run(format!("brand-kit.json: {e}")))?;
    let entry = kit
        .get("production")
        .and_then(|p| p.get(identity))
        .filter(|e| e.is_object())
        .ok_or_else(|| ProductionError::Run(format!("no brand `{identity}` in the kit")))?;
    let repo = entry.get("repo").and_then(Value::as_str).unwrap_or_default();
    let logo_rel = entry.get("logo").and_then(Value::as_str).unwrap_or_default();
    let logo = (!logo_rel.is_empty() && !logo_rel.starts_with('/') && !logo_rel.contains(".."))
        .then(|| {
            let mut roots = vec![kun.to_path_buf()];
            if pack::valid_name(repo) {
                roots.push(home.join(repo));
            }
            roots.into_iter().map(|r| r.join(logo_rel)).find(|p| p.is_file())
        })
        .flatten();
    let logo_data = logo.as_deref().and_then(|p| thumb_of(p, home));
    let palette: serde_json::Map<String, Value> = entry
        .get("video")
        .and_then(Value::as_object)
        .map(|o| o.iter().filter(|(_, v)| v.as_str().is_some_and(|s| s.starts_with('#'))).map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    Ok(json!({
        "identity": identity,
        "status": entry.get("status").cloned().unwrap_or(Value::Null),
        "logo": logo.map(|p| p.display().to_string()),
        "logoData": logo_data,
        "fonts": entry.get("fonts").cloned().unwrap_or_else(|| json!({})),
        "palette": palette,
        "voice": entry.get("voice").cloned().unwrap_or(Value::Null),
    }))
}

/// `production_brand` — read once per identity, then served from memory.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn production_brand(identity: String) -> Result<Value, ProductionError> {
    if let Some(v) = BRANDS.lock().unwrap_or_else(PoisonError::into_inner).get(&identity) {
        return Ok(v.clone());
    }
    let kun = pack::kun_dir().map_err(|e| ProductionError::Run(e.to_string()))?;
    let home = dirs::home_dir().ok_or_else(|| ProductionError::Run("could not find the home folder".to_owned()))?;
    let v = brand_in(&kun, &home, &identity)?;
    BRANDS.lock().unwrap_or_else(PoisonError::into_inner).insert(identity, v.clone());
    Ok(v)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, padded.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn mime_of(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// An image under `home` as a data URL; `None` for anything else or a file over [`MAX_THUMB_BYTES`].
#[must_use]
pub fn thumb_of(path: &Path, home: &Path) -> Option<String> {
    let real = path.canonicalize().ok()?;
    if !real.starts_with(home.canonicalize().ok()?) {
        return None;
    }
    let mime = mime_of(&real)?;
    let meta = fs::metadata(&real).ok()?;
    if !meta.is_file() || meta.len() > MAX_THUMB_BYTES {
        return None;
    }
    let bytes = fs::read(&real).ok()?;
    let mut url = String::from("data:");
    let _ = write!(url, "{mime};base64,{}", base64(&bytes));
    Some(url)
}

/// `production_thumb` — a scene's image as a data URL, or null (not an image, too big, outside home).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn production_thumb(path: String) -> Option<String> {
    let home = dirs::home_dir()?;
    thumb_of(Path::new(&path), &home)
}

/// Which of `paths` are files under `home` (same order), for the Scenes tab's "missing" mark.
#[must_use]
pub fn exist_in(paths: &[String], home: &Path) -> Vec<bool> {
    let home = home.canonicalize().ok();
    paths
        .iter()
        .map(|p| {
            let real = Path::new(p).canonicalize().ok();
            matches!((&real, &home), (Some(r), Some(h)) if r.starts_with(h) && r.is_file())
        })
        .collect()
}

/// `production_exist` — whether each asset path is on disk.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn production_exist(paths: Vec<String>) -> Vec<bool> {
    dirs::home_dir().map_or_else(|| vec![false; paths.len()], |h| exist_in(&paths, &h))
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("production")
        .invoke_handler(tauri::generate_handler![production_list, production_status, production_brand, production_thumb, production_exist, production_lock, production_save_plan])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
