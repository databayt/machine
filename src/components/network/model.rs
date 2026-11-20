//! The wire shapes of `src/lib/contract.ts` (health, heartbeat, messages, team), mirrored
//! with serde in camelCase. Keep them in step with the contract.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One thing the machine checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "network/types.ts")]
pub struct HealthItem {
    /// `driver`, `catalog`, `repo:<name>`, `workspace`, `disk`, `kun`, `sprint`.
    pub key: String,
    pub ok: bool,
    pub detail: String,
    /// True when the machine fixed it itself on this check.
    pub healed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct HealthReport {
    pub checked_at: String,
    pub items: Vec<HealthItem>,
}

/// Whether a clone is kept or swept when its sprint ends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "network/types.ts")]
pub enum RepoMode {
    #[default]
    Keep,
    Sprint,
}

/// A clone on this computer, as the heartbeat reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "network/types.ts")]
pub struct LocalRepo {
    pub name: String,
    pub mode: RepoMode,
    pub branch: String,
    pub behind: u32,
    pub ahead: u32,
    pub dirty: bool,
}

/// POST `/api/machine/heartbeat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    pub app_version: String,
    pub os: String,
    pub health: HealthReport,
    pub repos: Vec<LocalRepo>,
    pub current_task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatResponse {
    #[serde(default)]
    pub unread: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "network/types.ts")]
pub enum MessageKind {
    Note,
    Job,
    Nudge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct MessageBody {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct MachineMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: MessageKind,
    pub body: MessageBody,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "network/types.ts")]
pub struct MessagesResponse {
    pub messages: Vec<MachineMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct MachineStatus {
    pub login: String,
    pub name: String,
    pub os: String,
    pub departments: Vec<String>,
    pub last_seen_at: Option<String>,
    pub health_ok: Option<bool>,
    pub current_task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "network/types.ts")]
pub struct TeamResponse {
    pub machines: Vec<MachineStatus>,
}

/// POST `/api/machine/messages`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingMessage<'a> {
    pub to: &'a str,
    pub kind: MessageKind,
    pub body: MessageBody,
}

/// The agent CLIs a task may name with `driver=` and a receipt may report.
pub const DRIVERS: [&str; 5] = ["claude", "codex", "opencode", "agy", "hermes"];

/// POST `/api/machine/receipt`: what a run did. Only the task's holder may post it; kun comments
/// it on the issue (marker `<!-- kun:receipt -->`) and sets `status:review`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Receipt {
    pub id: String,
    pub driver: String,
    pub model: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub commits: Vec<String>,
    pub artifacts: Vec<String>,
    /// 1 to 4000 characters.
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

impl Receipt {
    /// The server's schema, checked before the call: a valid task id, a known driver, a model,
    /// a summary of 1 to 4000 characters and a `next` of at most 1000.
    ///
    /// # Errors
    /// The first rule broken, as a short sentence.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !super::valid_task_id(&self.id) {
            return Err("task id is invalid");
        }
        if !DRIVERS.contains(&self.driver.as_str()) {
            return Err("driver is unknown");
        }
        let model = self.model.trim().chars().count();
        if model == 0 || model > 100 {
            return Err("model must be 1 to 100 characters");
        }
        let summary = self.summary.trim().chars().count();
        if summary == 0 || summary > 4000 {
            return Err("summary must be 1 to 4000 characters");
        }
        if self.next.as_deref().is_some_and(|n| n.trim().chars().count() > 1000) {
            return Err("next must be at most 1000 characters");
        }
        if self.commits.len() > 100 || self.artifacts.len() > 100 {
            return Err("at most 100 commits and 100 artifacts");
        }
        Ok(())
    }
}
