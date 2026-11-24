//! The wire shapes of the task API in `src/lib/contract.ts`, mirrored with serde in
//! camelCase, plus the gate's verdict. Keep them in step with the contract.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::network::model::DRIVERS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "tasks/types.ts")]
pub enum TaskStatus {
    #[serde(rename = "todo")]
    Todo,
    #[serde(rename = "in-progress")]
    InProgress,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "done")]
    Done,
}

/// An agent CLI name, or a web seat as `web:<id>` (`web:chatgpt`), from the block's `driver=`;
/// anything unknown reads as `None`.
fn known_driver<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let raw = Option::<String>::deserialize(d)?;
    Ok(raw.map(|s| s.trim().to_lowercase()).filter(|s| {
        DRIVERS.contains(&s.as_str()) || s.strip_prefix("web:").is_some_and(crate::web::valid_seat_id)
    }))
}

/// The parsed `<!-- kun: repos=…; items=…; after=…; driver=…; formula=…; stage=… -->` block of an issue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export, export_to = "tasks/types.ts")]
pub struct KunBlock {
    pub repos: Vec<String>,
    pub items: Vec<String>,
    pub after: Vec<String>,
    /// `claude`, `codex`, `opencode`, `agy` or `hermes`, or a web seat `web:<id>` (`web:chatgpt`); `None` = the machine profile's default.
    #[serde(deserialize_with = "known_driver")]
    pub driver: Option<String>,
    /// The formula the issue was filled from (`formula=`), when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub formula: Option<String>,
    /// The produce stage the issue belongs to (`stage=`), when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct Task {
    /// `databayt/<repo>#<number>`
    pub id: String,
    pub repo: String,
    pub number: u32,
    pub title: String,
    pub url: String,
    pub department: String,
    pub status: TaskStatus,
    /// The kun login holding it, from the `holder:<login>` label.
    pub holder: Option<String>,
    pub priority: Option<u8>,
    pub sprint: Option<String>,
    /// Queue position: priority (P0 first), then milestone (current sprint first), then age.
    pub order: u32,
    /// The `run:auto` label: eligible for opt-in automatic pickup.
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub kun: KunBlock,
    /// The `after` ids still open: non-empty means blocked.
    #[serde(default)]
    pub after_open: Vec<String>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub updated_at: String,
    /// The login the issue is assigned to (`for:<login>` label): an order given to a person, who may
    /// be outside the task's department. Absent on older servers.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub assignee: Option<String>,
    /// The film the order belongs to (its film issue, `databayt/<repo>#<n>`), when it is a work order.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub film: Option<String>,
    /// The formula id the issue was filled from, e.g. `video.order.blender`.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub formula: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct Sprint {
    pub title: String,
    pub goal: Option<String>,
    pub ends_at: Option<String>,
}

/// GET `/api/machine/tasks?department=<d>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct TasksResponse {
    pub sprint: Option<Sprint>,
    /// Held by or queued for this login, in queue order.
    pub mine: Vec<Task>,
    /// Every open task of the department, in queue order.
    pub board: Vec<Task>,
    /// Tasks this login holds in progress across all departments.
    pub in_progress: u32,
    pub wip_limit: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "tasks/types.ts")]
pub enum ReadyState {
    /// Start it.
    Ready,
    /// Something to fetch first: a repo to clone or pull, an item to install.
    Missing,
    /// Not yours to start yet: waiting on another task, held, WIP full, or outside the profile.
    Blocked,
}

/// What the gate says about one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct Readiness {
    pub state: ReadyState,
    /// Why, as `repo:<name>`, `behind:<name>`, `item:<id>`, `after:<id>`, `held:<login>`,
    /// `wip`, `profile:<name>`, `doctor:<core step>`.
    pub missing: Vec<String>,
    /// The doctor is orange: this many extras are missing. Starting is still allowed.
    pub doctor_extras: u32,
    /// The doctor has not looked yet (unknown). Starting is allowed.
    pub doctor_checking: bool,
}

/// The answer of `tasks(department)`: the server's response plus a verdict per task id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TasksView {
    #[serde(flatten)]
    pub tasks: TasksResponse,
    pub readiness: std::collections::BTreeMap<String, Readiness>,
}

/// One issue comment, as `GET /api/machine/task?id=` returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "tasks/types.ts")]
pub struct TaskComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
    /// The body starts with `<!-- kun:receipt -->`.
    #[serde(default)]
    pub receipt: bool,
}

/// GET `/api/machine/task?id=<id>`: the task plus its last comments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task: Task,
    #[serde(default)]
    pub comments: Vec<TaskComment>,
}

/// What `TASK.md` adds to the issue: the recent comments and the machine-wide state excerpt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub comments: Vec<TaskComment>,
    pub aware: Option<String>,
}
