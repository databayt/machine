//! readiness — the types: kun's catalog, the state file, the run stream and what the page lists.
//!
//! The catalog (`<kun>/.claude/machine/readiness.json`) and the runner's NDJSON are kun's contract;
//! every field the page does not need is optional here so a newer kun never breaks an older app.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Text in both languages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Localized {
    #[serde(default)]
    pub ar: String,
    #[serde(default)]
    pub en: String,
}

/// One stage of the road from joining the org to a fully configured computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Stage {
    pub id: String,
    pub title: Option<Localized>,
}

impl<'de> Deserialize<'de> for Stage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Id(String),
            Full {
                id: String,
                #[serde(default)]
                title: Option<Localized>,
            },
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Id(id) => Self { id, title: None },
            Raw::Full { id, title } => Self { id, title },
        })
    }
}

/// How the probe runs; `probe` is the id of one of the runner's checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Check {
    #[serde(default)]
    pub probe: String,
    #[serde(default)]
    pub timeout_s: u32,
}

/// What a person can do about a failing step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "readiness/types.ts")]
pub enum FixKind {
    /// The runner fixes it.
    Auto,
    /// A command the person runs in Terminal.
    Terminal,
    /// A page the person opens.
    Link,
    /// A message to an owner who does it.
    Request,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Fix {
    pub kind: FixKind,
    #[serde(default)]
    pub cmd: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// A recurring update the `--maintain` run applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Update {
    #[serde(default)]
    pub cmd: String,
    #[serde(default)]
    pub cadence: String,
    #[serde(default)]
    pub auto: bool,
}

/// One thing that must be true for a computer to be ready.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Step {
    pub id: String,
    pub stage: String,
    #[serde(default)]
    pub title: Localized,
    /// Empty or `*` means every OS (`mac`, `linux`, `win`).
    #[serde(default)]
    pub os: Vec<String>,
    /// Empty or `*` means every department.
    #[serde(default)]
    pub departments: Vec<String>,
    /// Empty or `*` means every role (`engineer`, `member`).
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub after: Vec<String>,
    /// Only a person can do it.
    #[serde(default)]
    pub human: bool,
    #[serde(default)]
    pub check: Option<Check>,
    #[serde(default)]
    pub fix: Option<Fix>,
    #[serde(default)]
    pub update: Option<Update>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct Catalog {
    #[serde(default)]
    #[ts(type = "string | number | null")]
    pub version: Value,
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// Where a step stands. `pending` is ours: no run has reported on it yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "readiness/types.ts")]
pub enum StepStatus {
    #[default]
    Pending,
    Pass,
    Fail,
    Skip,
    Blocked,
    Fixing,
    Fixed,
    Human,
    UpdateAvailable,
    Updated,
    /// A status a newer runner invented.
    #[serde(other)]
    Unknown,
}

/// One line of the state file, `~/.claude/state/readiness.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct StateStep {
    pub id: String,
    #[serde(default)]
    pub status: StepStatus,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    #[ts(type = "string | number | null")]
    pub at: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "readiness/types.ts")]
pub struct ReadinessState {
    #[serde(default)]
    #[ts(type = "string | number | null")]
    pub catalog_version: Value,
    #[serde(default)]
    #[ts(type = "string | number | null")]
    pub at: Value,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub failing: Vec<String>,
    #[serde(default)]
    pub human: Vec<String>,
    #[serde(default)]
    pub steps: Vec<StateStep>,
}

/// What the runner does: look, look and fix, or update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "readiness/types.ts")]
pub enum Mode {
    Check,
    Complete,
    Maintain,
}

impl Mode {
    pub(super) fn flag(self) -> &'static str {
        match self {
            Self::Check => "--check",
            Self::Complete => "--complete",
            Self::Maintain => "--maintain",
        }
    }
}

/// One line of the runner's NDJSON, plus two the app adds (`log`, `error`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export, export_to = "readiness/types.ts")]
pub enum RunEvent {
    #[serde(rename_all = "camelCase")]
    Start {
        #[serde(default)]
        mode: String,
        #[serde(default)]
        #[ts(type = "string | number | null")]
        catalog_version: Value,
        #[serde(default)]
        total: u32,
    },
    Progress {
        #[serde(default)]
        #[ts(type = "string | number | null")]
        phase: Value,
        #[serde(default)]
        #[ts(type = "string | number | null")]
        of: Value,
        #[serde(default)]
        label: String,
    },
    Step {
        id: String,
        #[serde(default)]
        stage: String,
        status: StepStatus,
        #[serde(default)]
        detail: String,
    },
    Pass {
        #[serde(default)]
        n: u32,
    },
    Done {
        #[serde(default)]
        percent: f64,
        #[serde(default)]
        passed: u32,
        #[serde(default)]
        applicable: u32,
        #[serde(default)]
        failing: Vec<String>,
        #[serde(default)]
        human: Vec<String>,
    },
    /// A line the runner printed that is not an event.
    Log { line: String },
    /// The run itself failed (no node, a crash, a timeout).
    Error { message: String },
}

/// How a run ended.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct RunSummary {
    pub percent: f64,
    pub passed: u32,
    pub applicable: u32,
    pub failing: Vec<String>,
    pub human: Vec<String>,
    pub cancelled: bool,
}

/// A catalog step with where it stands.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct StepView {
    #[serde(flatten)]
    pub step: Step,
    pub status: StepStatus,
    pub detail: String,
}

/// Everything the Readiness page draws.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "readiness/types.ts")]
pub struct ReadinessView {
    /// False when kun has no catalog on this computer yet.
    pub catalog_found: bool,
    #[ts(type = "string | number | null")]
    pub catalog_version: Value,
    pub stages: Vec<Stage>,
    pub steps: Vec<StepView>,
    pub percent: f64,
    pub failing: Vec<String>,
    pub human: Vec<String>,
    #[ts(type = "string | number | null")]
    pub at: Value,
    /// Whether a run has ever finished and left a state file.
    pub has_state: bool,
    /// A state file at 100%.
    pub complete: bool,
    pub os: String,
    pub role: String,
    pub departments: Vec<String>,
}

/// What `readiness_fix` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct FixOutcome {
    pub kind: FixKind,
    pub ok: bool,
    pub message: Option<String>,
}

#[allow(clippy::struct_excessive_bools)]
/// One row of the Agents & tools tab: a probe merged with its catalog step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "readiness/types.ts")]
pub struct AgentToolRow {
    pub id: String,
    /// `agent` or `tool`.
    pub kind: String,
    pub label: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub needs_key: bool,
    pub step_id: Option<String>,
    pub status: Option<StepStatus>,
    pub detail: String,
    pub can_fix: bool,
    pub can_update: bool,
}

/// What the heartbeat tells kun about readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessBeat {
    pub percent: u32,
    pub failing: Vec<String>,
    pub catalog_version: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadinessError {
    #[error("kun has no readiness catalog here ({0}); update the kun checkout")]
    NoCatalog(String),
    #[error("the readiness catalog is damaged: {0}")]
    BadCatalog(String),
    #[error("not a step id: {0}")]
    BadId(String),
    #[error("no such step: {0}")]
    UnknownStep(String),
    #[error("step {0} has no fix")]
    NoFix(String),
    #[error("another readiness run is going; wait or cancel it")]
    Busy,
    #[error("node is not installed, so the provision script cannot run")]
    NoNode,
    #[error("the provision script is missing: {0}")]
    NoScript(String),
    #[error("could not start {0}: {1}")]
    Spawn(&'static str, String),
    #[error("the run exited with {0} and no result")]
    Failed(String),
    #[error("{0}")]
    Unsupported(&'static str),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Other(String),
}

impl Serialize for ReadinessError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
