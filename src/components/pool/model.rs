//! The wire shapes of kun's pool routes (`src/lib/machine/contract.ts`), mirrored with serde in
//! camelCase. Keep them in step with the contract.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "pool/types.ts")]
pub struct PoolFile {
    pub name: String,
    pub key: String,
    #[ts(type = "number")]
    pub size: u64,
    pub content_type: String,
    /// A presigned GET (10 minutes); only on a done job of mine. Never leaves Rust.
    #[serde(default, skip_serializing)]
    #[ts(skip)]
    pub get_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "pool/types.ts")]
pub struct PoolNeeds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub ram_bytes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub disk_bytes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "pool/types.ts")]
pub struct PoolJob {
    pub id: String,
    pub kind: String,
    #[ts(type = "Record<string, unknown>")]
    pub params: serde_json::Value,
    pub requester: String,
    pub target: Option<String>,
    pub donor: Option<String>,
    /// `uploading` · `queued` · `running` · `done` · `failed` · `cancelled`.
    pub status: String,
    pub inputs: Vec<PoolFile>,
    pub outputs: Option<Vec<PoolFile>>,
    pub needs: Option<PoolNeeds>,
    pub progress: f64,
    pub log: String,
    pub claims: u32,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pool/types.ts")]
pub struct PoolList {
    /// My jobs, newest first.
    pub mine: Vec<PoolJob>,
    /// Queued jobs I may claim (empty unless sharing is on).
    pub queue: Vec<PoolJob>,
}

/// One presigned link, for an input (requester) or an output (donor).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolUpload {
    pub name: String,
    pub put_url: String,
    #[serde(default)]
    pub content_type: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateResponse {
    pub job: PoolJob,
    pub uploads: Vec<PoolUpload>,
}

#[derive(Debug, Deserialize)]
pub struct Claimed {
    pub job: PoolJob,
    pub inputs: Vec<PoolFile>,
}

#[derive(Debug, Deserialize)]
pub struct ProgressAnswer {
    #[serde(default)]
    pub cancelled: bool,
}

#[derive(Debug, Deserialize)]
pub struct DoneAnswer {
    #[serde(default)]
    pub uploads: Vec<PoolUpload>,
}

/// The file facts a requester or donor declares before uploading.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSpec {
    pub name: String,
    pub size: u64,
    pub content_type: String,
}

/// What the donor loop is doing, for the Machines page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "pool/types.ts")]
pub struct DonorStatus {
    /// `off` · `waiting` (sharing on, not eligible) · `eligible` · `running`.
    pub state: String,
    /// Why it is not eligible: `no-kinds` · `battery` · `active` · `unknown` · `no-token` · `doctor`.
    pub reason: Option<String>,
    /// Kinds whose tools are installed here, so the page can enable only those.
    pub kinds_available: Vec<String>,
    pub on_ac: Option<bool>,
    pub idle_secs: Option<f64>,
    pub job: Option<RunningJob>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "pool/types.ts")]
pub struct RunningJob {
    pub id: String,
    pub kind: String,
    pub progress: f64,
}

/// Upload progress while a requester's files go up.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "pool/types.ts")]
pub struct UploadEvent {
    pub id: String,
    pub name: String,
    pub index: u32,
    pub count: u32,
    #[ts(type = "number")]
    pub sent: u64,
    #[ts(type = "number")]
    pub total: u64,
}
