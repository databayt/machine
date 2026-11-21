//! readiness — turns the runner's `--uninstall` events into what the page shows.
//!
//! Only the removal phase counts: the runner's `step` events up to its first `pass` event. The
//! final check pass that follows reports every step again (a removed one as `skip`), and must not
//! overwrite what the uninstall did.

use std::collections::BTreeMap;

use super::model::{
    Catalog, Handoff, RunEvent, StepNote, StepStatus, UninstallOutcome,
};

/// Sorts the events seen for the `requested` ids into removed / blocked / failed / handoff.
/// A handoff's `cmd` and `keystone` come from the catalog, never from the event text.
pub(super) fn outcome(
    requested: &[String],
    events: &[RunEvent],
    catalog: Option<&Catalog>,
    cancelled: bool,
) -> UninstallOutcome {
    let mut last: BTreeMap<&str, (StepStatus, &str)> = BTreeMap::new();
    for event in events {
        match event {
            RunEvent::Pass { .. } => break,
            RunEvent::Step {
                id, status, detail, ..
            } if requested.iter().any(|r| r == id) => {
                if matches!(
                    status,
                    StepStatus::Removed
                        | StepStatus::Blocked
                        | StepStatus::Fail
                        | StepStatus::Human
                ) {
                    last.insert(id.as_str(), (*status, detail.as_str()));
                }
            }
            _ => {}
        }
    }
    let mut out = UninstallOutcome {
        cancelled,
        ..UninstallOutcome::default()
    };
    // In the order the person asked.
    for id in requested {
        let Some((status, detail)) = last.get(id.as_str()) else {
            continue;
        };
        let note = || StepNote {
            id: id.clone(),
            detail: (*detail).to_owned(),
        };
        match status {
            StepStatus::Removed => out.removed.push(id.clone()),
            StepStatus::Blocked => out.blocked.push(note()),
            StepStatus::Fail => out.failed.push(note()),
            StepStatus::Human => {
                let un = catalog
                    .and_then(|c| c.steps.iter().find(|s| &s.id == id))
                    .and_then(|s| s.uninstall.as_ref());
                out.handoff.push(Handoff {
                    id: id.clone(),
                    cmd: un.map(|u| u.cmd.clone()).unwrap_or_default(),
                    keystone: un.is_some_and(|u| u.keystone),
                });
            }
            _ => {}
        }
    }
    out
}
