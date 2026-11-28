//! readiness — loads kun's catalog and the state file, filters the steps that apply to this
//! computer and puts them in the order a person works through them: by stage, and never a step
//! before what it comes `after`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use super::model::{
    Catalog, GateBeat, GateLevel, GateTier, GateView, ReadinessBeat, ReadinessError, ReadinessState, ReadinessView,
    Step, StepStatus, StepView,
};

/// `<kun>/.claude/machine/readiness.json`.
pub(super) fn catalog_path(kun: &Path) -> PathBuf {
    kun.join(".claude/machine/readiness.json")
}

/// `<claude>/state/readiness.json`.
pub(super) fn state_path(claude: &Path) -> PathBuf {
    claude.join("state/readiness.json")
}

/// Reads kun's catalog.
///
/// # Errors
/// When the file is missing or is not a catalog.
pub(super) fn load(kun: &Path) -> Result<Catalog, ReadinessError> {
    let path = catalog_path(kun);
    let text = fs::read_to_string(&path)
        .map_err(|_| ReadinessError::NoCatalog(path.display().to_string()))?;
    parse(&text)
}

pub(super) fn parse(text: &str) -> Result<Catalog, ReadinessError> {
    serde_json::from_str(text).map_err(|e| ReadinessError::BadCatalog(e.to_string()))
}

/// The state file, or `None` when no run has finished yet or the file is unreadable.
pub(super) fn load_state(claude: &Path) -> Option<ReadinessState> {
    serde_json::from_str(&fs::read_to_string(state_path(claude)).ok()?).ok()
}

/// Who is asking: the OS, the person's departments and their role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Scope {
    pub(super) os: String,
    pub(super) departments: Vec<String>,
    pub(super) role: String,
}

impl Scope {
    /// This computer, this profile.
    pub(super) fn current() -> Self {
        let resolved = crate::profile::current().ok();
        let departments = resolved
            .as_ref()
            .map(|r| {
                r.profile
                    .departments
                    .iter()
                    .map(|d| d.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        let engineer =
            resolved.is_some_and(|r| r.profile.access.code == crate::profile::CodeAccess::Write);
        Self {
            os: os_name().to_owned(),
            departments,
            role: if engineer { "engineer" } else { "member" }.to_owned(),
        }
    }
}

/// The OS name kun's catalog uses.
pub(super) fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "mac",
        "windows" => "win",
        other => other,
    }
}

fn os_matches(listed: &str, os: &str) -> bool {
    let norm = |s: &str| match s.to_ascii_lowercase().as_str() {
        "mac" | "macos" | "darwin" => "mac".to_owned(),
        "win" | "win32" | "windows" => "win".to_owned(),
        o => o.to_owned(),
    };
    listed == "*" || norm(listed) == norm(os)
}

fn any_or_all(listed: &[String], matches: impl Fn(&str) -> bool) -> bool {
    listed.is_empty() || listed.iter().any(|l| l == "*" || matches(l))
}

/// Whether a step is for this person on this computer.
pub(super) fn applies(step: &Step, scope: &Scope) -> bool {
    any_or_all(&step.os, |o| os_matches(o, &scope.os))
        && any_or_all(&step.departments, |d| {
            scope.departments.iter().any(|s| s == d)
        })
        && any_or_all(&step.roles, |r| r == scope.role)
}

/// The steps that apply, by stage in the catalog's stage order, and never before what they come
/// `after`. A cycle or an unknown `after` cannot stall the list: the first step left goes next.
pub(super) fn ordered(catalog: &Catalog, scope: &Scope) -> Vec<Step> {
    let rank: BTreeMap<&str, usize> = catalog
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut left: Vec<(usize, &Step)> = catalog
        .steps
        .iter()
        .filter(|s| applies(s, scope))
        .enumerate()
        .collect();
    left.sort_by_key(|(i, s)| {
        (
            rank.get(s.stage.as_str()).copied().unwrap_or(usize::MAX),
            *i,
        )
    });
    let present: BTreeSet<&str> = left.iter().map(|(_, s)| s.id.as_str()).collect();
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let pick = left
            .iter()
            .position(|(_, s)| {
                s.after
                    .iter()
                    .all(|a| !present.contains(a.as_str()) || done.contains(a))
            })
            .unwrap_or(0);
        let (_, step) = left.remove(pick);
        done.insert(step.id.clone());
        out.push(step.clone());
    }
    out
}

/// What the Readiness page draws.
pub(super) fn view(
    catalog: Option<&Catalog>,
    scope: &Scope,
    state: Option<&ReadinessState>,
) -> ReadinessView {
    let steps: Vec<StepView> = catalog
        .map(|c| ordered(c, scope))
        .unwrap_or_default()
        .into_iter()
        .map(|step| {
            let (status, detail) = state
                .and_then(|s| s.steps.iter().find(|x| x.id == step.id))
                .map_or((StepStatus::Pending, String::new()), |x| {
                    (x.status, x.detail.clone())
                });
            StepView {
                step,
                status,
                detail,
            }
        })
        .collect();
    let percent = state.map_or(0.0, |s| s.percent);
    ReadinessView {
        catalog_found: catalog.is_some(),
        catalog_version: catalog.map(|c| c.version.clone()).unwrap_or_default(),
        stages: catalog.map(|c| c.stages.clone()).unwrap_or_default(),
        steps,
        percent,
        failing: state.map(|s| s.failing.clone()).unwrap_or_default(),
        human: state.map(|s| s.human.clone()).unwrap_or_default(),
        at: state.map(|s| s.at.clone()).unwrap_or_default(),
        has_state: state.is_some(),
        complete: state.is_some_and(|s| s.percent >= 100.0),
        os: scope.os.clone(),
        role: scope.role.clone(),
        departments: scope.departments.clone(),
        removed: state.map(|s| s.removed.clone()).unwrap_or_default(),
        gate: catalog.map(|c| gate_view(state, c, scope)),
    }
}

/// Whether the gate covers this step for this person: the step is flagged `gate`, applies to this
/// computer, and, when it names `gateFor`, one of those departments is the person's.
pub(super) fn gate_applies(step: &Step, scope: &Scope) -> bool {
    step.gate.is_some()
        && applies(step, scope)
        && step
            .gate_for
            .as_ref()
            .is_none_or(|f| f.iter().any(|d| d == "*" || scope.departments.contains(d)))
}

/// The ids of the gate steps for this person, in catalog stage order.
pub(super) fn gate_ids(catalog: &Catalog, scope: &Scope) -> Vec<String> {
    ordered(catalog, scope)
        .into_iter()
        .filter(|s| gate_applies(s, scope))
        .map(|s| s.id)
        .collect()
}

/// The doctor's verdict from the latest readiness state alone: no probe runs. A gate step passes
/// when the runner last saw it `pass`, `fixed`, `updated` or `update-available`; a `skip` passes
/// unless the person removed the step on purpose. A failing core step is red, only failing extras
/// orange, none green. No state yet is unknown: nothing has been looked at, so nothing is blocked.
pub(super) fn gate_view(
    state: Option<&ReadinessState>,
    catalog: &Catalog,
    scope: &Scope,
) -> GateView {
    let gate: Vec<Step> = ordered(catalog, scope)
        .into_iter()
        .filter(|s| gate_applies(s, scope))
        .collect();
    let steps: Vec<String> = gate.iter().map(|s| s.id.clone()).collect();
    let Some(state) = state else {
        return GateView { steps, ..GateView::default() };
    };
    let failing_of = |tier: GateTier| -> Vec<String> {
        gate.iter()
            .filter(|s| s.gate == Some(tier) && !gate_step_ok(Some(state), &s.id))
            .map(|s| s.id.clone())
            .collect()
    };
    let core_failing = failing_of(GateTier::Core);
    let extra_failing = failing_of(GateTier::Extra);
    let level = if !core_failing.is_empty() {
        GateLevel::Red
    } else if !extra_failing.is_empty() {
        GateLevel::Orange
    } else {
        GateLevel::Green
    };
    GateView {
        level,
        pass: level != GateLevel::Red,
        failing: core_failing.iter().chain(&extra_failing).cloned().collect(),
        core_failing,
        extra_failing,
        steps,
        at: state.at.as_str().unwrap_or_default().to_owned(),
    }
}

fn gate_step_ok(state: Option<&ReadinessState>, id: &str) -> bool {
    let Some(state) = state else { return false };
    match state.steps.iter().find(|s| s.id == id).map(|s| s.status) {
        Some(
            StepStatus::Pass | StepStatus::Fixed | StepStatus::Updated | StepStatus::UpdateAvailable,
        ) => true,
        Some(StepStatus::Skip) => !state.removed.iter().any(|r| r == id),
        _ => false,
    }
}

fn version_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// Whether the doctor should run by itself now: nothing has been looked at yet (no state), kun's
/// catalog changed since the state was written, or the app is not the version that last ran the
/// doctor (an update, or the first launch ever).
pub(super) fn needs_doctor(
    state: Option<&ReadinessState>,
    catalog: &Catalog,
    last_app: Option<&str>,
    app: &str,
) -> bool {
    let Some(state) = state else { return true };
    version_text(&state.catalog_version) != version_text(&catalog.version) || last_app != Some(app)
}

/// `<machine dir>/doctor-run.json`: the app version that last ran the doctor on its own.
pub(super) fn doctor_marker_path(dir: &Path) -> PathBuf {
    dir.join("doctor-run.json")
}

/// The app version in the marker, if there is one.
pub(super) fn read_doctor_marker(path: &Path) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    v.get("appVersion")?.as_str().map(str::to_owned)
}

/// Records that the doctor ran under `app`.
pub(super) fn write_doctor_marker(path: &Path, app: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::json!({ "appVersion": app }).to_string())
}

/// `secs` since the epoch as `YYYY-MM-DDTHH:MM:SSZ`.
pub(super) fn iso(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let day_of_year = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let d = day_of_year - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// The gate as the heartbeat carries it; `now` stands in for a state with no timestamp. `None`
/// while the level is unknown: the heartbeat omits it.
pub(super) fn gate_beat(gate: GateView, now: u64) -> Option<GateBeat> {
    Some(GateBeat {
        level: gate.level.word()?,
        pass: gate.pass,
        failing: gate.failing,
        at: if gate.at.is_empty() { iso(now) } else { gate.at },
    })
}

/// The heartbeat's readiness field; `None` before any run left a state file.
pub(super) fn beat(
    state: &ReadinessState,
    catalog: Option<&Catalog>,
    scope: &Scope,
    now: u64,
) -> ReadinessBeat {
    ReadinessBeat {
        gate: catalog.and_then(|c| gate_beat(gate_view(Some(state), c, scope), now)),
        // Clamped, then rounded: the heartbeat carries a whole percent.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        percent: state.percent.clamp(0.0, 100.0).round() as u32,
        failing: state.failing.clone(),
        catalog_version: state.catalog_version.clone(),
    }
}

/// A step id: what the runner accepts after `--only`, nothing a shell or a flag could use.
pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with(['-', '.'])
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
}
