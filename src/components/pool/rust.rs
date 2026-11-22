//! pool — the shared compute pool. A machine that opted in lends its idle time for heavy jobs
//! (transcribe, render, encode); any machine can send one. Fixed templates only, never a Claude
//! run: see `params.rs`. The donor loop runs in the background while sharing is on; the requester
//! side is the commands below. Every call goes to kun; machines never talk to each other.

// Byte counts to f64 for the rules: far below 2^53.
#![allow(clippy::cast_precision_loss)]

use std::{
    sync::{Arc, LazyLock, Mutex, PoisonError},
    time::Duration,
};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{
    Runtime,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
};

#[path = "donor.rs"]
pub mod donor;
#[path = "model.rs"]
pub mod model;
#[path = "params.rs"]
pub mod params;

use donor::{Ctl, Refused, Snapshot, ToolPaths, Tools};
use model::{CreateResponse, DonorStatus, FileSpec, PoolJob, PoolList, PoolNeeds, RunningJob, UploadEvent};

use crate::{
    network::{
        api::{self, Api, ApiError, HttpApi},
        resources::{self, RealProbe},
    },
    tasks::upload,
};

/// How often an eligible donor looks at the queue.
const POLL: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Local(String),
}

impl Serialize for PoolError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

static STATE: LazyLock<Mutex<DonorStatus>> = LazyLock::new(|| Mutex::new(DonorStatus { state: "off".into(), ..DonorStatus::default() }));
/// One job at a time, and no status flicker while it runs.
static RUNNING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

fn set_state(f: impl FnOnce(&mut DonorStatus)) {
    f(&mut STATE.lock().unwrap_or_else(PoisonError::into_inner));
}

fn tool_paths() -> ToolPaths {
    ToolPaths { whisper: crate::tools::find("whisper-cli"), ffmpeg: crate::tools::find("ffmpeg"), blender: crate::tools::find("blender") }
}

/// Reads the machine (blocking: shell-outs and a directory walk) into what the rules look at.
fn snapshot() -> Snapshot {
    let share = crate::settings::load().map(|s| s.share).unwrap_or_default();
    let paths = tool_paths();
    let p = RealProbe;
    let models = donor::find_models(&donor::model_dirs()).iter().filter_map(|m| m.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
    Snapshot {
        share,
        on_ac: resources::power(&p).map(|w| w.on_ac),
        idle_secs: resources::idle_secs(&p),
        ram_free: resources::ram_free(&p).map(|n| n as f64),
        disk_free: crate::network::host::Host::disk_free_bytes(&crate::network::host::RealHost).map(|n| n as f64),
        has_gpu: None,
        tools: Tools { whisper: paths.whisper.is_some(), ffmpeg: paths.ffmpeg.is_some(), blender: paths.blender.is_some() },
        models,
    }
}

/// The donor state for a snapshot (not while a job runs).
fn status_for(s: &Snapshot) -> DonorStatus {
    let (state, reason) = match donor::gate(s) {
        Ok(()) => ("eligible", None),
        Err(donor::Why::Off) => ("off", None),
        Err(w) => ("waiting", Some(w.code().to_owned())),
    };
    DonorStatus {
        state: state.into(),
        reason,
        kinds_available: donor::kinds_available(&s.tools),
        on_ac: s.on_ac,
        idle_secs: s.idle_secs,
        job: None,
        last_error: STATE.lock().unwrap_or_else(PoisonError::into_inner).last_error.clone(),
    }
}

async fn evaluate() -> Option<(Snapshot, DonorStatus)> {
    let snap = tokio::task::spawn_blocking(snapshot).await.ok()?;
    let status = status_for(&snap);
    Some((snap, status))
}

/// One look at the queue: claim the first job that fits, and run it to its end.
async fn pass() {
    let Ok(_one) = RUNNING.try_lock() else { return };
    let Some((snap, status)) = evaluate().await else { return };
    let eligible = status.state == "eligible";
    set_state(|s| *s = status);
    if !eligible {
        return;
    }
    let Ok(api) = HttpApi::from_process() else { return };
    let list: PoolList = match api::get_json(&api, "/api/machine/pool").await {
        Ok(l) => l,
        Err(ApiError::NoToken) => {
            set_state(|s| {
                s.state = "waiting".into();
                s.reason = Some("no-token".into());
            });
            return;
        }
        Err(e) => {
            set_state(|s| s.last_error = Some(e.to_string()));
            return;
        }
    };
    for job in list.queue.iter().filter(|j| donor::job_fits(j, &snap).is_ok()) {
        match donor::claim(&api, &job.id).await {
            Ok(claimed) => {
                run_one(&api, claimed).await;
                return;
            }
            Err(Refused::Reason(r)) if r == "doctor" => {
                set_state(|s| {
                    s.state = "waiting".into();
                    s.reason = Some("doctor".into());
                });
                return;
            }
            Err(Refused::Reason(_)) => {}
            Err(Refused::Api(e)) => {
                set_state(|s| s.last_error = Some(e));
                return;
            }
        }
    }
}

async fn run_one(api: &HttpApi, claimed: model::Claimed) {
    let (id, kind) = (claimed.job.id.clone(), claimed.job.kind.clone());
    set_state(|s| {
        s.state = "running".into();
        s.reason = None;
        s.job = Some(RunningJob { id: id.clone(), kind, progress: 0.0 });
    });
    let ctl = Arc::new(Ctl::default());
    // The status line follows the run's progress.
    let follow = {
        let ctl = Arc::clone(&ctl);
        tokio::spawn(async move {
            while !ctl.finished.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_secs(2)).await;
                set_state(|s| {
                    if let Some(j) = s.job.as_mut() {
                        j.progress = ctl.progress();
                    }
                });
            }
        })
    };
    let Ok(client) = upload::put_client() else { return };
    let finish = donor::run_claimed(api, &client, claimed, &tool_paths(), Arc::clone(&ctl), || resources::idle_secs(&RealProbe)).await;
    ctl.finished.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = follow.await;
    tracing::info!("pool job {id}: {finish:?}");
    set_state(|s| {
        s.job = None;
        s.state = "waiting".into();
        s.last_error = match finish {
            donor::Finish::Failed(e) => Some(e),
            _ => None,
        };
    });
}

async fn background() {
    tokio::time::sleep(Duration::from_secs(20)).await;
    // A crash can leave a job folder behind; nothing runs yet, so clear them all.
    if let Some(root) = donor::pool_root() {
        let _ = std::fs::remove_dir_all(root);
    }
    let mut tick = tokio::time::interval(POLL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        pass().await;
    }
}

// ---- requester commands --------------------------------------------------------------------------

/// `pool_jobs` — my jobs and, when sharing, the queue.
#[tauri::command]
async fn pool_jobs() -> Result<PoolList, PoolError> {
    Ok(api::get_json(&HttpApi::from_process()?, "/api/machine/pool").await?)
}

/// What a kind roughly needs, so kun can steer it to a machine with room.
fn needs_for(kind: &str, input_bytes: u64) -> PoolNeeds {
    let gib = 1_073_741_824.0;
    #[allow(clippy::cast_precision_loss)]
    let disk = input_bytes as f64 * 3.0;
    PoolNeeds { ram_bytes: Some(match kind { "render" => 4.0 * gib, "transcribe" => 2.0 * gib, _ => gib }), gpu: None, disk_bytes: Some(disk) }
}

/// Creates the job, uploads each input (progress on `on_event`), then marks it ready. Any failure
/// cancels the job so nobody waits for it.
async fn submit_with(api: &HttpApi, kind: &str, params: Value, target: Option<String>, paths: &[String], on_event: Arc<dyn Fn(UploadEvent) + Send + Sync>) -> Result<PoolJob, PoolError> {
    params::validate(kind, &params).map_err(PoolError::Invalid)?;
    if target.as_deref().is_some_and(|t| !crate::profile::valid_login(t)) {
        return Err(PoolError::Invalid("the target must be a kun login".into()));
    }
    let home = dirs::home_dir().ok_or_else(|| PoolError::Local("no home folder".into()))?;
    let picked = upload::pick_files(paths, &upload::allowed_roots(&home)).map_err(|e| PoolError::Invalid(e.to_string()))?;
    if picked.len() > 10 {
        return Err(PoolError::Invalid("at most 10 files per job".into()));
    }
    let total: u64 = picked.iter().map(|p| p.file.size).sum();
    let files: Vec<FileSpec> = picked.iter().map(|p| FileSpec { name: p.file.name.clone(), size: p.file.size, content_type: p.file.content_type.clone() }).collect();
    let body = json!({ "kind": kind, "params": params, "target": target, "files": files, "needs": needs_for(kind, total) });
    let created: CreateResponse = api::post_json(api, "/api/machine/pool", &body).await?;
    let id = created.job.id.clone();
    let client = upload::put_client().map_err(|e| PoolError::Local(e.to_string()))?;
    let count = u32::try_from(picked.len()).unwrap_or(u32::MAX);
    for (i, p) in picked.iter().enumerate() {
        let Some(up) = created.uploads.iter().find(|u| u.name == p.file.name) else {
            cancel_quietly(api, &id).await;
            return Err(PoolError::Local("kun gave no upload link for a file".into()));
        };
        let target = upload::UploadTarget { name: p.file.name.clone(), key: String::new(), put_url: up.put_url.clone(), public_url: String::new(), content_type: if up.content_type.is_empty() { p.file.content_type.clone() } else { up.content_type.clone() } };
        let (name, total_bytes, jid) = (p.file.name.clone(), p.file.size, id.clone());
        let index = u32::try_from(i).unwrap_or(u32::MAX);
        let sink = Arc::clone(&on_event);
        let emit: Arc<dyn Fn(u64) + Send + Sync> = Arc::new(move |sent| sink(UploadEvent { id: jid.clone(), name: name.clone(), index, count, sent, total: total_bytes }));
        if let Err(e) = upload::put_file(&client, &target, &p.path, p.file.size, emit).await {
            cancel_quietly(api, &id).await;
            return Err(PoolError::Local(e.to_string()));
        }
    }
    if let Err(e) = api::post_json::<Value>(api, "/api/machine/pool/ready", &json!({ "id": id })).await {
        cancel_quietly(api, &id).await;
        return Err(e.into());
    }
    let mut job = created.job;
    job.status = "queued".into();
    Ok(job)
}

async fn cancel_quietly(api: &impl Api, id: &str) {
    let _ = api::post_json::<Value>(api, "/api/machine/pool/cancel", &json!({ "id": id })).await;
}

/// `pool_submit` — send a job: create, upload the files (progress events), mark ready.
#[tauri::command]
async fn pool_submit(kind: String, params: Value, target: Option<String>, paths: Vec<String>, on_event: Channel<UploadEvent>) -> Result<PoolJob, PoolError> {
    submit_with(&HttpApi::from_process()?, &kind, params, target, &paths, Arc::new(move |e| {
        let _ = on_event.send(e);
    }))
    .await
}

/// `pool_cancel` — cancel one of my jobs (queued or running).
#[tauri::command]
async fn pool_cancel(id: String) -> Result<(), PoolError> {
    if !donor::valid_id(&id) {
        return Err(PoolError::Invalid("job id is invalid".into()));
    }
    api::post_json::<Value>(&HttpApi::from_process()?, "/api/machine/pool/cancel", &json!({ "id": id })).await?;
    Ok(())
}

/// Opens the folder that holds `file` with the file selected where the system can.
fn reveal(file: &std::path::Path) {
    let _ = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-R").arg(file).spawn()
    } else if cfg!(windows) {
        std::process::Command::new("explorer").arg(format!("/select,{}", file.display())).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(file.parent().unwrap_or(file)).spawn()
    };
}

/// `pool_download` — saves a done job's outputs to `~/Downloads/kun-pool/<id>/` and reveals them.
#[tauri::command]
async fn pool_download(id: String) -> Result<String, PoolError> {
    if !donor::valid_id(&id) {
        return Err(PoolError::Invalid("job id is invalid".into()));
    }
    let api = HttpApi::from_process()?;
    let list: PoolList = api::get_json(&api, "/api/machine/pool").await?;
    let job = list.mine.into_iter().find(|j| j.id == id && j.status == "done").ok_or_else(|| PoolError::Invalid("that job is not done".into()))?;
    let dir = dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads"))).ok_or_else(|| PoolError::Local("no Downloads folder".into()))?.join("kun-pool").join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| PoolError::Local(e.to_string()))?;
    let client = upload::put_client().map_err(|e| PoolError::Local(e.to_string()))?;
    let mut first = None;
    for f in job.outputs.unwrap_or_default() {
        let url = f.get_url.ok_or_else(|| PoolError::Local("an output has no link".into()))?;
        let dest = dir.join(params::safe_name(&f.name));
        donor::download(&client, &url, &dest).await.map_err(|e| PoolError::Local(format!("{e:?}")))?;
        first.get_or_insert(dest);
    }
    if let Some(f) = first {
        reveal(&f);
    }
    Ok(dir.display().to_string())
}

/// `pool_status` — what the donor side is doing, and why it is not eligible when it is not.
#[tauri::command]
async fn pool_status() -> DonorStatus {
    let running = STATE.lock().unwrap_or_else(PoisonError::into_inner).state == "running";
    if !running && let Some((_, mut status)) = evaluate().await {
        let keep = STATE.lock().unwrap_or_else(PoisonError::into_inner).reason.clone().filter(|r| r == "doctor" || r == "no-token");
        if status.state == "eligible" && keep.is_some() {
            status.state = "waiting".into();
            status.reason = keep;
        }
        set_state(|s| *s = status);
    }
    STATE.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("pool")
        .invoke_handler(tauri::generate_handler![pool_jobs, pool_submit, pool_cancel, pool_download, pool_status])
        .setup(|_app, _api| {
            tauri::async_runtime::spawn(background());
            Ok(())
        })
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
