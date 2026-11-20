//! readiness — the Agents & tools tab: the drivers `detect()` finds and the tools `probe()` finds,
//! each merged with the catalog step (agents and tools stages) that installs and updates it.

use crate::{driver::DriverStatus, tools::ToolStatus};

use super::model::{AgentToolRow, Catalog, FixKind, ReadinessState, Step, StepStatus};

fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn stage_kind(step: &Step) -> Option<&'static str> {
    let stage = step.stage.to_ascii_lowercase();
    if stage.contains("agent") {
        Some("agent")
    } else if stage.contains("tool") {
        Some("tool")
    } else {
        None
    }
}

/// The step that installs `probe_id`: an agents/tools step whose id starts with it
/// (`claude` → `claude.cli`, `whisper-cli` → `tool.whisper-cli`). The plainest id wins.
fn step_for<'a>(catalog: &'a Catalog, probe_id: &str) -> Option<&'a Step> {
    let want = norm(probe_id);
    catalog
        .steps
        .iter()
        .filter(|s| stage_kind(s).is_some())
        .filter(|s| {
            let id = norm(&s.id);
            let bare = norm(
                s.id.strip_prefix("tool.")
                    .or_else(|| s.id.strip_prefix("agent."))
                    .unwrap_or(&s.id),
            );
            id == want || bare.starts_with(&want)
        })
        .min_by_key(|s| {
            let id = s.id.to_ascii_lowercase();
            if id == probe_id || id == format!("tool.{probe_id}") {
                0
            } else if id.rsplit('.').next().is_some_and(|e| e == "cli") {
                1
            } else {
                2
            }
        })
}

fn status_of(state: Option<&ReadinessState>, id: &str) -> (Option<StepStatus>, String) {
    state
        .and_then(|s| s.steps.iter().find(|x| x.id == id))
        .map_or((None, String::new()), |x| {
            (Some(x.status), x.detail.clone())
        })
}

fn row(
    id: &str,
    kind: &str,
    label: &str,
    probe: (bool, Option<String>, Option<String>, bool),
    step: Option<&Step>,
    state: Option<&ReadinessState>,
) -> AgentToolRow {
    let (status, detail) = step.map_or((None, String::new()), |s| status_of(state, &s.id));
    AgentToolRow {
        id: id.to_owned(),
        kind: kind.to_owned(),
        label: label.to_owned(),
        installed: probe.0,
        path: probe.1,
        version: probe.2,
        needs_key: probe.3,
        step_id: step.map(|s| s.id.clone()),
        status,
        detail,
        can_fix: step.is_some_and(|s| {
            s.fix
                .as_ref()
                .is_some_and(|f| f.kind != FixKind::Request || f.message.is_some())
        }),
        can_update: step.is_some_and(|s| s.update.is_some()),
    }
}

/// The merged list: every driver, every tool, then catalog steps no probe covers.
#[must_use]
pub(super) fn merge(
    drivers: &[DriverStatus],
    tools: &[ToolStatus],
    catalog: Option<&Catalog>,
    state: Option<&ReadinessState>,
) -> Vec<AgentToolRow> {
    let mut used: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    for d in drivers {
        let bin = d.kind.binary();
        let step = catalog.and_then(|c| step_for(c, bin));
        if let Some(s) = step {
            used.push(s.id.clone());
        }
        rows.push(row(
            bin,
            "agent",
            bin,
            (d.installed, d.path.clone(), None, false),
            step,
            state,
        ));
    }
    for t in tools {
        let step = catalog.and_then(|c| step_for(c, &t.id));
        if let Some(s) = step {
            used.push(s.id.clone());
        }
        rows.push(row(
            &t.id,
            "tool",
            &t.label,
            (t.installed, t.path.clone(), t.version.clone(), t.needs_key),
            step,
            state,
        ));
    }
    if let Some(c) = catalog {
        for s in c.steps.iter().filter(|s| !used.contains(&s.id)) {
            let Some(kind) = stage_kind(s) else { continue };
            let (status, _) = status_of(state, &s.id);
            let installed = matches!(
                status,
                Some(StepStatus::Pass | StepStatus::Fixed | StepStatus::Updated)
            );
            let title = if s.title.en.is_empty() {
                &s.id
            } else {
                &s.title.en
            };
            rows.push(row(
                &s.id,
                kind,
                title,
                (installed, None, None, false),
                Some(s),
                state,
            ));
        }
    }
    rows
}
