//! Submitting a work order: the person's files go to the CDN, then the receipt goes on the issue.
//!
//! kun's `POST /api/machine/upload` answers with one presigned `PUT` link per file (no cloud key
//! ever reaches this machine). Each file is streamed to its link from here with a progress line,
//! hashed on the way, and the run receipt then carries the public links and the hashes. The PUT
//! never carries the machine token: the link is the authority.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use futures_util::stream;
use reqwest::{Body, header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use ts_rs::TS;

use super::TaskError;
use crate::network::{
    api::{self, Api},
    model::Receipt,
};

/// The most files one submission may carry.
pub const MAX_FILES: usize = 20;
/// How much of a file is read at a time.
const CHUNK: usize = 256 * 1024;
/// A single upload may run this long (a film's footage is large).
const PUT_TIMEOUT: Duration = Duration::from_secs(6 * 3600);

/// One file named in the upload request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct UploadFile {
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub content_type: String,
}

/// POST `/api/machine/upload`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct UploadRequest {
    /// The task, `databayt/<repo>#<n>`.
    pub id: String,
    pub files: Vec<UploadFile>,
}

/// Where one file goes: the object key, the presigned link to `PUT` it to, and the link it will
/// be public at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct UploadTarget {
    pub name: String,
    pub key: String,
    pub put_url: String,
    pub public_url: String,
    pub content_type: String,
}

/// The answer to the upload request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct UploadResponse {
    pub uploads: Vec<UploadTarget>,
}

/// What the chat is told while a submission runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub enum SubmitEvent {
    /// The upload links are in; `count` files follow.
    Prepared { count: u32 },
    /// Bytes of file `index` (0-based) sent so far.
    Progress {
        index: u32,
        count: u32,
        name: String,
        #[ts(type = "number")]
        sent: u64,
        #[ts(type = "number")]
        total: u64,
    },
    /// One file is up and hashed.
    Uploaded { index: u32, count: u32, name: String, url: String },
    /// Every file is up; the receipt is being posted.
    Posting,
}

/// A file the person chose, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub path: PathBuf,
    pub file: UploadFile,
}

/// The content type a CDN object is served with, from the file's extension.
#[must_use]
pub fn content_type_of(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e).to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" | "srt" | "vtt" => "text/plain",
        "md" => "text/markdown",
        _ => "application/octet-stream",
    }
}

/// Where a submitted file may come from: the home folder and the usual external-drive folders.
#[must_use]
pub fn allowed_roots(home: &Path) -> Vec<PathBuf> {
    [home, Path::new("/Volumes"), Path::new("/mnt"), Path::new("/media")]
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect()
}

fn valid_file_name(n: &str) -> bool {
    !n.is_empty() && n.chars().count() <= 200 && !n.contains(['/', '\\']) && !n.chars().any(char::is_control)
}

/// Checks the chosen paths: absolute, existing regular files under an allowed root and outside any
/// hidden folder (`~/.ssh`, `~/.kun`), distinct names, at most [`MAX_FILES`].
///
/// # Errors
/// [`TaskError::Invalid`] naming the first rule broken.
pub fn pick_files(paths: &[String], roots: &[PathBuf]) -> Result<Vec<Picked>, TaskError> {
    if paths.is_empty() {
        return Err(TaskError::Invalid("choose at least one file"));
    }
    if paths.len() > MAX_FILES {
        return Err(TaskError::Invalid("at most 20 files at a time"));
    }
    let mut out: Vec<Picked> = Vec::new();
    for raw in paths {
        let given = PathBuf::from(raw);
        if !given.is_absolute() {
            return Err(TaskError::Invalid("a file path must be absolute"));
        }
        let path = given.canonicalize().map_err(|_| TaskError::Invalid("a chosen file was not found"))?;
        let meta = std::fs::metadata(&path).map_err(|_| TaskError::Invalid("a chosen file cannot be read"))?;
        if !meta.is_file() {
            return Err(TaskError::Invalid("only files can be submitted, not folders"));
        }
        let Some(rel) = roots.iter().find_map(|r| path.strip_prefix(r).ok()) else {
            return Err(TaskError::Invalid("a file must be in your home folder or on an external drive"));
        };
        if rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
            return Err(TaskError::Invalid("files inside hidden folders are never submitted"));
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !valid_file_name(&name) {
            return Err(TaskError::Invalid("a file name is not valid"));
        }
        if out.iter().any(|p| p.file.name == name) {
            return Err(TaskError::Invalid("two chosen files have the same name"));
        }
        let content_type = content_type_of(&name).to_owned();
        out.push(Picked { path, file: UploadFile { name, size: meta.len(), content_type } });
    }
    Ok(out)
}

/// A link a file may be `PUT` to: https, or a loopback address (tests, a local object store).
fn put_url_ok(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://127.0.0.1") || u.starts_with("http://localhost")
}

/// The answer of the upload route, checked and put in the order of `files`.
///
/// # Errors
/// [`TaskError::Refused`] with the server's reason when it says `ok: false`, [`TaskError::Local`]
/// when the answer is not one link per file.
pub fn targets_from(answer: &serde_json::Value, files: &[UploadFile]) -> Result<Vec<UploadTarget>, TaskError> {
    if answer.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        let word = answer.get("error").and_then(|e| e.as_str()).unwrap_or("refused");
        let detail = answer.get("detail").and_then(|e| e.as_str());
        return Err(TaskError::Refused(detail.map_or_else(|| word.to_owned(), |d| format!("{word}: {d}"))));
    }
    let bad = |why: &str| TaskError::Local(format!("kun's upload answer is not usable: {why}"));
    let res: UploadResponse = serde_json::from_value(answer.clone()).map_err(|_| bad("no uploads"))?;
    let mut out = Vec::with_capacity(files.len());
    for f in files {
        let t = res.uploads.iter().find(|t| t.name == f.name).ok_or_else(|| bad("a file has no link"))?;
        if !put_url_ok(&t.put_url) || !t.public_url.starts_with("https://") {
            return Err(bad("a link is not https"));
        }
        out.push(UploadTarget {
            content_type: if t.content_type.is_empty() { f.content_type.clone() } else { t.content_type.clone() },
            ..t.clone()
        });
    }
    Ok(out)
}

/// Asks kun for one upload link per file.
///
/// # Errors
/// The server's reason when it refuses; see [`targets_from`].
pub async fn request_uploads(api: &impl Api, id: &str, files: &[UploadFile]) -> Result<Vec<UploadTarget>, TaskError> {
    let req = UploadRequest { id: id.to_owned(), files: files.to_vec() };
    let answer: serde_json::Value = api::post_json(api, "/api/machine/upload", &req).await.map_err(super::refusal)?;
    targets_from(&answer, files)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

struct Reading {
    file: tokio::fs::File,
    hasher: Arc<Mutex<Sha256>>,
    sent: u64,
    last: u64,
    total: u64,
    progress: Arc<dyn Fn(u64) + Send + Sync>,
}

/// Streams one file to its presigned link, hashing as it goes, and returns its sha256 (hex).
/// `progress` is called with the bytes sent, about once per percent and at the end.
///
/// # Errors
/// [`TaskError::Local`] when the file cannot be read or the link refuses (the status and the start
/// of its answer are in the message).
pub async fn put_file(
    client: &reqwest::Client,
    target: &UploadTarget,
    path: &Path,
    size: u64,
    progress: Arc<dyn Fn(u64) + Send + Sync>,
) -> Result<String, TaskError> {
    let local = |e: String| TaskError::Local(e);
    if !put_url_ok(&target.put_url) {
        return Err(local("the upload link is not https".to_owned()));
    }
    let file = tokio::fs::File::open(path).await.map_err(|e| local(format!("could not read {}: {e}", target.name)))?;
    let hasher = Arc::new(Mutex::new(Sha256::new()));
    let state = Reading { file, hasher: hasher.clone(), sent: 0, last: 0, total: size, progress };
    let step = (size / 100).max(CHUNK as u64);
    let body = Body::wrap_stream(stream::unfold(state, move |mut st| async move {
        let mut buf = vec![0u8; CHUNK];
        match st.file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                st.hasher.lock().unwrap_or_else(PoisonError::into_inner).update(&buf);
                st.sent += n as u64;
                if st.sent - st.last >= step || st.sent >= st.total {
                    st.last = st.sent;
                    (st.progress)(st.sent);
                }
                Some((Ok::<Vec<u8>, std::io::Error>(buf), st))
            }
            Err(e) => Some((Err(e), st)),
        }
    }));
    let res = client
        .put(&target.put_url)
        .header(header::CONTENT_TYPE, &target.content_type)
        .header(header::CONTENT_LENGTH, size)
        .timeout(PUT_TIMEOUT)
        .body(body)
        .send()
        .await
        .map_err(|e| local(format!("upload of {} failed: {}", target.name, e.without_url())))?;
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        let snippet: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect();
        return Err(local(format!("the upload of {} was refused ({}): {snippet}", target.name, status.as_u16())));
    }
    let digest = hasher.lock().unwrap_or_else(PoisonError::into_inner).clone().finalize();
    Ok(hex(&digest))
}

/// The plain HTTP client for presigned links: no token, no redirects.
///
/// # Errors
/// When the client cannot be built.
pub fn put_client() -> Result<reqwest::Client, TaskError> {
    reqwest::Client::builder()
        .user_agent(concat!("kun-machine/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| TaskError::Local(e.to_string()))
}

/// What a submission carries besides the files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notes {
    pub driver: String,
    pub model: String,
    pub duration_ms: u64,
    pub summary: String,
    pub next: Option<String>,
    pub formula: Option<String>,
    pub stage: Option<String>,
}

/// The receipt for uploaded files: the public links, their hashes in the same order.
#[must_use]
pub fn receipt_for(id: &str, notes: &Notes, artifacts: Vec<String>, hashes: Vec<String>) -> Receipt {
    let clean = |s: &Option<String>| s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    Receipt {
        id: id.to_owned(),
        driver: notes.driver.clone(),
        model: notes.model.clone(),
        duration_ms: notes.duration_ms,
        commits: Vec::new(),
        artifacts,
        summary: notes.summary.trim().to_owned(),
        next: clean(&notes.next),
        formula: clean(&notes.formula),
        stage: clean(&notes.stage),
        hashes: Some(hashes),
    }
}

/// The whole submission: links, files, then the receipt. `on_event` is the chat's progress line.
/// Returns the public links.
///
/// # Errors
/// The server's reason on a refused upload request or receipt; a message on a failed upload. Nothing
/// is posted unless every file is up.
pub async fn submit_with(
    api: &impl Api,
    client: &reqwest::Client,
    id: &str,
    picked: &[Picked],
    notes: &Notes,
    on_event: Arc<dyn Fn(SubmitEvent) + Send + Sync>,
) -> Result<Vec<String>, TaskError> {
    if notes.summary.trim().is_empty() {
        return Err(TaskError::Invalid("write one line about the work first"));
    }
    let files: Vec<UploadFile> = picked.iter().map(|p| p.file.clone()).collect();
    let targets = request_uploads(api, id, &files).await?;
    let count = u32::try_from(picked.len()).unwrap_or(u32::MAX);
    on_event(SubmitEvent::Prepared { count });
    let (mut urls, mut hashes) = (Vec::new(), Vec::new());
    for (i, (p, t)) in picked.iter().zip(&targets).enumerate() {
        let index = u32::try_from(i).unwrap_or(u32::MAX);
        let (name, total) = (p.file.name.clone(), p.file.size);
        let emit = on_event.clone();
        let progress_name = name.clone();
        let progress: Arc<dyn Fn(u64) + Send + Sync> = Arc::new(move |sent| {
            emit(SubmitEvent::Progress { index, count, name: progress_name.clone(), sent, total });
        });
        let hash = put_file(client, t, &p.path, p.file.size, progress).await?;
        on_event(SubmitEvent::Uploaded { index, count, name, url: t.public_url.clone() });
        urls.push(t.public_url.clone());
        hashes.push(hash);
    }
    on_event(SubmitEvent::Posting);
    super::receipt_with(api, &receipt_for(id, notes, urls.clone(), hashes)).await?;
    Ok(urls)
}

#[cfg(test)]
#[path = "upload_test.rs"]
mod tests;
