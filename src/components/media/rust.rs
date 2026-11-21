//! media — the quick-reference media library of the selected repository, for the chat composer.
//!
//! `media_library(repo)` reads the repo checkout's first `media-manifest.json` under `src/`
//! (hogwarts keeps one at `src/components/docs/`), maps its images and videos to [`MediaItem`]s, and
//! adds the approved videos of that brand from `cdn.databayt.org/video/index.json` when it answers.
//! Results are cached in memory per repo for ten minutes. The CDN is best effort: offline or a 404
//! just means fewer items.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, PoisonError},
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;
use walkdir::WalkDir;

const MANIFEST: &str = "media-manifest.json";
const CDN_INDEX: &str = "https://cdn.databayt.org/video/index.json";
const TTL: Duration = Duration::from_secs(600);
const MAX_DEPTH: usize = 6;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("repository name is invalid")]
    BadRepo,
    #[error("could not find the repositories folder: {0}")]
    Root(String),
}

impl Serialize for MediaError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// One image or video the person can attach as a reference.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "media/types.ts")]
pub struct MediaItem {
    pub id: String,
    /// `image` or `video`.
    pub kind: String,
    pub title_ar: String,
    pub title_en: String,
    /// What the grid shows: a video's poster, an image's smallest rendition. Empty when there is none.
    pub thumb: String,
    /// What the message cites: the h264 mp4, or the largest image.
    pub url: String,
    pub width: u32,
    pub height: u32,
    /// Seconds; 0 for images.
    pub duration: f64,
    /// `ar`, `en`, or empty when the item serves both.
    pub lang: String,
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn dim(v: &Value, key: &str) -> u32 {
    v.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

/// `{"1600": url, "2400": url}` as (width, url), narrowest first.
fn renditions(v: &Value, key: &str) -> Vec<(u32, String)> {
    let mut all: Vec<(u32, String)> = v
        .get(key)
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(w, u)| Some((w.parse().ok()?, u.as_str()?.to_string())))
        .collect();
    all.sort_by_key(|(w, _)| *w);
    all
}

fn lang_of(id: &str, url: &str) -> String {
    let lower = id.to_lowercase();
    for l in ["ar", "en"] {
        if lower.ends_with(&format!("-{l}")) || url.contains(&format!("/{l}/")) {
            return l.to_string();
        }
    }
    String::new()
}

fn bilingual(v: &Value, key: &str) -> (String, String) {
    let o = v.get(key);
    let pick = |l: &str| {
        o.and_then(|o| o.get(l))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    (pick("ar"), pick("en"))
}

fn map_entry(id: &str, v: &Value) -> Option<MediaItem> {
    match v.get("kind").and_then(Value::as_str)? {
        "video" => {
            let sources = v.get("sources").and_then(Value::as_array)?;
            let src = |s: &Value| (text(s, "src"), text(s, "type"));
            let url = sources
                .iter()
                .map(src)
                .find(|(_, t)| t.contains("avc1"))
                .or_else(|| sources.first().map(src))
                .map(|(u, _)| u)
                .filter(|u| !u.is_empty())?;
            let (title_ar, title_en) = bilingual(v, "title");
            Some(MediaItem {
                id: id.to_string(),
                kind: "video".into(),
                title_ar,
                title_en,
                thumb: text(v, "poster"),
                lang: lang_of(id, &url),
                url,
                width: dim(v, "width"),
                height: dim(v, "height"),
                duration: v.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
            })
        }
        "image" => {
            // webp renders everywhere; avif is the fallback when a manifest ships only that.
            let mut all = renditions(v, "webp");
            if all.is_empty() {
                all = renditions(v, "avif");
            }
            let thumb = all.first()?.1.clone();
            let url = all.last()?.1.clone();
            let (title_ar, title_en) = bilingual(v, "alt");
            Some(MediaItem {
                id: id.to_string(),
                kind: "image".into(),
                title_ar,
                title_en,
                thumb,
                lang: lang_of(id, &url),
                url,
                width: dim(v, "width"),
                height: dim(v, "height"),
                duration: 0.0,
            })
        }
        _ => None,
    }
}

/// Maps a whole `media-manifest.json` (id → entry); entries of other kinds are skipped.
fn map_manifest(manifest: &Value) -> Vec<MediaItem> {
    manifest
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, v)| map_entry(id, v))
        .collect()
}

/// The approved videos of `repo` in the produce index (`video/index.json`).
fn map_cdn_index(index: &Value, repo: &str) -> Vec<MediaItem> {
    let urls_of = |v: &Value| -> Vec<String> {
        v.get("urls")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(_, u)| u.as_str().map(str::to_string))
            .collect()
    };
    index
        .get("videos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| {
            v.get("approved").and_then(Value::as_bool).unwrap_or(false)
                && text(v, "product").eq_ignore_ascii_case(repo)
        })
        .filter_map(|v| {
            let urls = urls_of(v);
            let url = urls.iter().find(|u| is_mp4(u))?.clone();
            let thumb = urls
                .iter()
                .find(|u| u.contains(".webp") || u.contains(".jpg") || u.contains(".png"))
                .cloned()
                .unwrap_or_default();
            let id = text(v, "id");
            let title = {
                let t = text(v, "title");
                if t.is_empty() { text(v, "feature") } else { t }
            };
            Some(MediaItem {
                lang: lang_of(&id, &url),
                id: format!("cdn:{id}"),
                kind: "video".into(),
                title_ar: title.clone(),
                title_en: title,
                thumb,
                url,
                width: 0,
                height: 0,
                duration: 0.0,
            })
        })
        .collect()
}

/// The first `media-manifest.json` under `<repo>/src`, skipping dependency and build folders.
fn find_manifest(repo_dir: &Path) -> Option<PathBuf> {
    WalkDir::new(repo_dir.join("src"))
        .max_depth(MAX_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "node_modules" && n != ".next"
        })
        .filter_map(Result::ok)
        .find(|e| e.file_type().is_file() && e.file_name() == MANIFEST)
        .map(walkdir::DirEntry::into_path)
}

fn is_mp4(url: &str) -> bool {
    Path::new(url)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
}

fn valid_repo(repo: &str) -> bool {
    !repo.is_empty()
        && repo.len() <= 64
        && !repo.starts_with('.')
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// The manifest's items for `repo` under `root`; nothing when the repo or its manifest is absent.
fn load_local(root: &Path, repo: &str) -> Vec<MediaItem> {
    let Some(path) = find_manifest(&root.join(repo)) else {
        return Vec::new();
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .map(|v| map_manifest(&v))
        .unwrap_or_default()
}

async fn load_cdn(repo: &str) -> Vec<MediaItem> {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(concat!("kun-machine/", env!("CARGO_PKG_VERSION")))
        .build()
    else {
        return Vec::new();
    };
    let Ok(res) = client.get(CDN_INDEX).send().await else {
        return Vec::new();
    };
    if !res.status().is_success() {
        return Vec::new();
    }
    res.json::<Value>()
        .await
        .map(|v| map_cdn_index(&v, repo))
        .unwrap_or_default()
}

type Cache = HashMap<String, (Instant, Vec<MediaItem>)>;
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn cached(repo: &str) -> Option<Vec<MediaItem>> {
    let map = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    map.get(repo)
        .filter(|(at, _)| at.elapsed() < TTL)
        .map(|(_, items)| items.clone())
}

/// `media_library` — the images and videos of `repo` that can be attached to a message.
#[tauri::command]
async fn media_library(repo: String) -> Result<Vec<MediaItem>, MediaError> {
    if !valid_repo(&repo) {
        return Err(MediaError::BadRepo);
    }
    if let Some(hit) = cached(&repo) {
        return Ok(hit);
    }
    let root = crate::repos::repos_root().map_err(|e| MediaError::Root(e.to_string()))?;
    let name = repo.clone();
    let mut items = tokio::task::spawn_blocking(move || load_local(&root, &name))
        .await
        .unwrap_or_default();
    let known: Vec<String> = items.iter().map(|i| i.url.clone()).collect();
    items.extend(
        load_cdn(&repo)
            .await
            .into_iter()
            .filter(|i| !known.contains(&i.url)),
    );
    CACHE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(repo, (Instant::now(), items.clone()));
    Ok(items)
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("media")
        .invoke_handler(tauri::generate_handler![media_library])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
