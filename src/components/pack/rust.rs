//! pack — builds the working folder an agent runs in.
//!
//! Without a machine profile, `~/.kun/machine/<dept>/` holds a `CLAUDE.md` and `AGENTS.md` generated from the
//! department's `prompt.md` (plus a pointer to kun's own rules), and the pack's skills
//! symlinked in, so every run starts with the right context and nothing else. The folder
//! is rebuilt from kun's checkout on every call, so it is idempotent and never drifts.
//!
//! With a profile (`machines/<login>.json`) the run lives in `~/Kun/<login>/` instead
//! (`workspace.rs`): CLAUDE.md, a generated KNOWLEDGE.md, the profile's skills, a `work/`
//! folder and the `--settings` file that enforces read-only code. The profile's public repos
//! are cloned into the repos root (a setting; `~/Kun/repos/` unless the installer chose another) and only ever fast-forwarded (the `repos` feature). Skill links
//! (`link.rs`) use junctions on Windows.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    Runtime,
    ipc::Channel,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;

use crate::driver::{DriverKind, PermissionMode};

#[path = "health.rs"]
pub mod health;
#[path = "link.rs"]
mod link;
#[path = "station.rs"]
pub mod station;
#[path = "workspace.rs"]
mod workspace;

pub use crate::repos::{SyncEvent, SyncState};
pub use workspace::{Workspace, ensure_workspace_in, ensure_workspace_with, settings_json};

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("department name is invalid")]
    BadName,
    #[error("could not find the home folder")]
    NoHome,
    #[error("no pack for `{0}` (expected {1})")]
    Missing(String, String),
    #[error("{0}: {1}")]
    Io(String, io::Error),
    #[error("repo name is invalid: {0}")]
    BadRepo(String),
    #[error("{0}")]
    Profile(String),
    #[error("this department has no route `{0}`")]
    UnknownRoute(String),
    #[error("{0}")]
    Station(String),
}

impl Serialize for PackError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> PackError + '_ {
    move |e| PackError::Io(path.display().to_string(), e)
}

/// A one-tap prompt shown under the greeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct QuickAction {
    /// The action's id in the pack (`video.reel.footage`), when it has one: the station log's key.
    pub id: Option<String>,
    pub label_ar: String,
    pub label_en: String,
    pub prompt: String,
    /// One or two lines saying what the action does, per language (the menu's workflow cards).
    #[serde(default)]
    pub description_en: String,
    #[serde(default)]
    pub description_ar: String,
    /// Repos this starter belongs to; empty = shown for any repo.
    #[serde(default)]
    pub repos: Vec<String>,
    /// The route (`pack.json` `routes[].id`) this action runs in; none = the pack's default.
    pub route: Option<String>,
}

/// A named place a home prompt runs: permission mode, model, effort, agent and an instruction
/// appended to every prompt sent through it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct Route {
    pub id: String,
    pub label_ar: String,
    pub label_en: String,
    pub description_ar: String,
    pub description_en: String,
    pub icon: Option<String>,
    /// `"plan"` or `"auto"`: Claude's permission mode for the run.
    pub mode: String,
    /// The engine lane the route runs in (`engine.json` `lanes`); its model and effort win over the route's own.
    pub lane: Option<String>,
    /// The model the run uses: the lane's when it resolves, else the route's own.
    pub model: Option<String>,
    pub effort: Option<String>,
    pub agent: Option<String>,
    /// Appended to the prompt as `\n\n<suffix>`; empty when none.
    pub suffix: String,
}

impl Route {
    /// The route's permission mode as the driver takes it.
    #[must_use]
    pub fn permission(&self) -> PermissionMode {
        PermissionMode::parse(Some(&self.mode))
    }
}

/// The pack's fixed frame, `formula.en`: the headings every finished prompt uses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct Formula {
    pub ar: String,
    pub en: String,
}

/// Parses the pack's raw `formula`; absent, malformed or blank gives none.
#[must_use]
pub fn parse_formula(raw: Option<&Value>) -> Option<Formula> {
    let f = serde_json::from_value::<Formula>(raw?.clone()).ok()?;
    let f = Formula { ar: f.ar.trim().to_owned(), en: f.en.trim().to_owned() };
    (!f.ar.is_empty() || !f.en.is_empty()).then_some(f)
}

/// The words that route a request to the department, per language.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct Keywords {
    pub en: Vec<String>,
    pub ar: Vec<String>,
}

/// What the UI shows about a department's pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct PackInfo {
    pub department: String,
    pub title_ar: String,
    pub title_en: String,
    pub default_driver: Option<DriverKind>,
    /// The fallback home options when the station's board is empty.
    pub quick_actions: Vec<QuickAction>,
    /// The pack's frame for a finished prompt (`formula`).
    pub formula: Option<Formula>,
    /// The lead agent (`lead`), the produce-style line (`line`) and the guard agent (`guard`).
    pub lead: Option<String>,
    pub line: Option<String>,
    pub guard: Option<String>,
    pub keywords: Keywords,
    /// Rules every prompt of the department carries.
    pub constraints: Vec<String>,
    /// Things refine reads first, one per entry (`gather`).
    pub gather: Vec<String>,
    /// Where a home prompt can run, in the pack's order (at most [`MAX_ROUTES`]).
    pub routes: Vec<Route>,
    /// The route the home selector starts on: the pack's choice when it exists, else the first.
    pub default_route: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawQuick {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    label_ar: String,
    #[serde(default)]
    label_en: String,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    description_en: String,
    #[serde(default)]
    description_ar: String,
    #[serde(default)]
    repos: Vec<String>,
    #[serde(default)]
    route: Option<String>,
}

/// One `routes[]` entry as written; parsed per entry so a bad one is skipped, not fatal.
#[derive(Debug, Default, Deserialize)]
struct RawRoute {
    #[serde(default)]
    id: String,
    #[serde(default, alias = "labelAr")]
    label_ar: String,
    #[serde(default, alias = "labelEn")]
    label_en: String,
    #[serde(default, alias = "descriptionAr")]
    description_ar: String,
    #[serde(default, alias = "descriptionEn")]
    description_en: String,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    lane: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    suffix: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawPack {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Value,
    #[serde(default)]
    title_ar: Option<String>,
    #[serde(default)]
    items: Vec<Value>,
    #[serde(default, rename = "defaultDriver")]
    default_driver: Option<DriverKind>,
    #[serde(default, rename = "quickActions")]
    quick_actions: Vec<RawQuick>,
    /// The frame for a finished prompt `{ar, en}` ([`parse_formula`]).
    #[serde(default)]
    formula: Option<Value>,
    #[serde(default)]
    lead: Option<String>,
    #[serde(default)]
    line: Option<String>,
    #[serde(default)]
    guard: Option<String>,
    #[serde(default)]
    keywords: Option<Value>,
    #[serde(default)]
    constraints: Vec<Value>,
    /// Things refine reads first; each entry is one thing to read.
    #[serde(default)]
    gather: Vec<Value>,
    /// kun.databayt.org home card ids this department shows (`public/r/kun/home.json`).
    #[serde(default)]
    cards: Vec<String>,
    /// The databayt repos this department works in.
    #[serde(default)]
    repos: Vec<String>,
    /// Extra folders under `$HOME` the department's chat runs may write (`--add-dir`).
    #[serde(default)]
    dirs: Vec<String>,
    /// A longer turn timeout for departments whose runs take hours (clamped on read).
    #[serde(default, rename = "timeoutSecs")]
    timeout_secs: Option<u32>,
    /// An automatic health check (and fix) for the department's home-page light.
    #[serde(default)]
    health: Option<health::HealthSpec>,
    /// Model, effort and agent per turn type (`plan` / `work`); validated by [`parse_run`].
    #[serde(default)]
    run: Option<Value>,
    /// Named run routes (`parse_routes`); entries are parsed one by one.
    #[serde(default)]
    routes: Vec<Value>,
    #[serde(default, rename = "defaultRoute")]
    default_route: Option<String>,
}

/// A department or skill name: lowercase-ish, no separators, no dots at the start.
#[must_use]
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with(['.', '-'])
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !s.contains("..")
}

/// kun's checkout: `KUN_DIR`, else `~/kun`.
///
/// # Errors
/// When there is no home folder and no override.
pub fn kun_dir() -> Result<PathBuf, PackError> {
    match std::env::var_os("KUN_DIR") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => dirs::home_dir()
            .map(|h| h.join("kun"))
            .ok_or(PackError::NoHome),
    }
}

/// machine's own folder: `KUN_HOME` (else `~/.kun`) + `/machine`.
///
/// # Errors
/// When there is no home folder and no override.
pub fn machine_dir() -> Result<PathBuf, PackError> {
    match std::env::var_os("KUN_HOME") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p).join("machine")),
        _ => dirs::home_dir()
            .map(|h| h.join(".kun/machine"))
            .ok_or(PackError::NoHome),
    }
}

fn write_if_changed(path: &Path, content: &str) -> Result<(), PackError> {
    if fs::read_to_string(path).is_ok_and(|old| old == content) {
        return Ok(());
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content).map_err(io_err(&tmp))?;
    fs::rename(&tmp, path).map_err(io_err(path))
}

/// Skill names an `items` list asks for: the catalog id `"skill:edit"`, `"skills/edit"`,
/// `"edit"` or `{"name": "edit"}`. Other catalog types (`"agent:lead"`) are not skills.
fn item_names(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter_map(|v| match v {
            Value::String(s) => match s.split_once(':') {
                Some(("skill", name)) => Some(name.to_owned()),
                Some(_) => None,
                None => s.rsplit('/').next().map(str::to_owned),
            },
            Value::Object(o) => o.get("name").and_then(Value::as_str).map(str::to_owned),
            _ => None,
        })
        .filter(|n| valid_name(n))
        .collect()
}

/// Agent names an `items` list asks for: only the catalog id `"agent:media-qa"`.
fn agent_names(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter_map(|v| match v {
            Value::String(s) => match s.split_once(':') {
                Some(("agent", name)) => Some(name.to_owned()),
                _ => None,
            },
            _ => None,
        })
        .filter(|n| valid_name(n))
        .collect()
}

/// One turn type's Claude settings from `pack.json` `run`; every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunSpec {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub agent: Option<String>,
}

/// The pack's `run` block: `plan` for plan-mode turns, `work` for the rest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunMeta {
    pub plan: Option<RunSpec>,
    pub work: Option<RunSpec>,
}

fn valid_model(m: &str) -> bool {
    if m.to_ascii_lowercase().contains("fable") {
        return false;
    }
    if matches!(m, "opus" | "sonnet" | "haiku") {
        return true;
    }
    m.strip_prefix("claude-")
        .and_then(|r| r.split_once('-'))
        .is_some_and(|(fam, rest)| {
            matches!(fam, "opus" | "sonnet" | "haiku")
                && !rest.is_empty()
                && rest
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

fn parse_run_spec(turn: &str, v: Option<&Value>) -> Option<RunSpec> {
    let o = v?.as_object()?;
    let field = |key: &str, ok: &dyn Fn(&str) -> bool| {
        let raw = o.get(key)?.as_str()?;
        if ok(raw) {
            Some(raw.to_owned())
        } else {
            tracing::warn!("pack run.{turn}.{key}: invalid value dropped");
            None
        }
    };
    let spec = RunSpec {
        model: field("model", &valid_model),
        effort: field("effort", &|e| {
            matches!(e, "low" | "medium" | "high" | "xhigh" | "max")
        }),
        agent: field("agent", &valid_name),
    };
    (spec != RunSpec::default()).then_some(spec)
}

fn parse_run(v: Option<&Value>) -> Option<RunMeta> {
    let v = v?;
    let run = RunMeta {
        plan: parse_run_spec("plan", v.get("plan")),
        work: parse_run_spec("work", v.get("work")),
    };
    (run != RunMeta::default()).then_some(run)
}

const VALID_EFFORT: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The most routes a pack may declare.
pub const MAX_ROUTES: usize = 6;
/// The longest route suffix, in characters; a longer one drops the route.
pub const MAX_SUFFIX_CHARS: usize = 400;

fn valid_route_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_agent(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// One engine lane: the model and effort a route resolves to (`engine.json` `lanes.<name>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneSpec {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// The engine's lanes by name. A lane without a model (`script`) or with a rejected one (`fable`)
/// leaves that field to the route's own value.
pub type Lanes = BTreeMap<String, LaneSpec>;

/// Reads `lanes` out of `engine.json`'s text. Names starting with `$` are the file's own notes.
#[must_use]
pub fn parse_lanes(text: &str) -> Lanes {
    let Some(map) = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.get("lanes").cloned())
        .and_then(|v| v.as_object().cloned())
    else {
        return Lanes::new();
    };
    map.iter()
        .filter(|(name, _)| !name.starts_with('$') && valid_route_id(name))
        .filter_map(|(name, v)| {
            let o = v.as_object()?;
            let model = o
                .get("model")
                .and_then(Value::as_str)
                .filter(|m| valid_model(m))
                .map(str::to_owned);
            let effort = o
                .get("effort")
                .and_then(Value::as_str)
                .filter(|e| VALID_EFFORT.contains(e))
                .map(str::to_owned);
            Some((name.clone(), LaneSpec { model, effort }))
        })
        .collect()
}

/// `<kun>/.claude/engine.json`'s lanes; a missing or damaged file gives none (routes keep their own values).
#[must_use]
pub fn read_lanes(kun: &Path) -> Lanes {
    fs::read_to_string(kun.join(".claude/engine.json"))
        .map(|t| parse_lanes(&t))
        .unwrap_or_default()
}

/// One route from its raw form; `None` (with a warning) when any field is invalid. A `lane` that
/// `lanes` knows sets the model and effort, over the route's own.
fn parse_route(v: &Value, lanes: &Lanes) -> Option<Route> {
    let r: RawRoute = serde_json::from_value(v.clone()).ok()?;
    let drop = |why: &str| {
        tracing::warn!("pack route `{}` dropped: {why}", r.id);
        None
    };
    if !valid_route_id(&r.id) {
        return drop("bad id");
    }
    if !matches!(r.mode.as_str(), "plan" | "auto") {
        return drop("mode must be plan or auto");
    }
    if r.model.as_deref().is_some_and(|m| !valid_model(m)) {
        return drop("invalid model");
    }
    if r.effort.as_deref().is_some_and(|e| !VALID_EFFORT.contains(&e)) {
        return drop("invalid effort");
    }
    if r.agent.as_deref().is_some_and(|a| !valid_agent(a)) {
        return drop("invalid agent");
    }
    if r.lane.as_deref().is_some_and(|l| !valid_route_id(l)) {
        return drop("invalid lane");
    }
    let lane = r.lane.as_deref().and_then(|l| lanes.get(l));
    let model = lane.and_then(|l| l.model.clone()).or_else(|| r.model.clone());
    let effort = lane.and_then(|l| l.effort.clone()).or_else(|| r.effort.clone());
    let suffix = r.suffix.as_deref().map_or("", str::trim);
    if suffix.chars().count() > MAX_SUFFIX_CHARS {
        return drop("suffix too long");
    }
    let label = |s: String| if s.trim().is_empty() { r.id.clone() } else { s };
    Some(Route {
        label_ar: label(r.label_ar.clone()),
        label_en: label(r.label_en.clone()),
        description_ar: r.description_ar.clone(),
        description_en: r.description_en.clone(),
        icon: r.icon.clone().filter(|i| !i.trim().is_empty()),
        mode: r.mode.clone(),
        lane: r.lane.clone(),
        model,
        effort,
        agent: r.agent.clone(),
        suffix: suffix.to_owned(),
        id: r.id.clone(),
    })
}

fn derived_route(id: &str, en: &str, ar: &str, mode: &str, spec: Option<&RunSpec>) -> Route {
    let spec = spec.cloned().unwrap_or_default();
    Route {
        id: id.to_owned(),
        label_ar: ar.to_owned(),
        label_en: en.to_owned(),
        description_ar: String::new(),
        description_en: String::new(),
        icon: None,
        mode: mode.to_owned(),
        lane: None,
        model: spec.model,
        effort: spec.effort,
        agent: spec.agent,
        suffix: String::new(),
    }
}

/// The pack's routes: its valid `routes` (unique ids, at most [`MAX_ROUTES`]); with none declared, a
/// pack that has a `run` block gets `plan` and `do` derived from it. Plus the default route id.
fn parse_routes(
    raw: &[Value],
    default: Option<&str>,
    run: Option<&RunMeta>,
    lanes: &Lanes,
) -> (Vec<Route>, Option<String>) {
    let mut routes: Vec<Route> = Vec::new();
    for route in raw.iter().filter_map(|v| parse_route(v, lanes)) {
        if routes.len() < MAX_ROUTES && !routes.iter().any(|r| r.id == route.id) {
            routes.push(route);
        }
    }
    let mut default = default.map(str::to_owned);
    if raw.is_empty()
        && let Some(run) = run
    {
        routes = vec![
            derived_route("plan", "Plan first", "خطة أولاً", "plan", run.plan.as_ref()),
            derived_route("do", "Do it now", "نفّذ الآن", "auto", run.work.as_ref()),
        ];
        default = Some("plan".to_owned());
    }
    let default = default
        .filter(|d| routes.iter().any(|r| &r.id == d))
        .or_else(|| routes.first().map(|r| r.id.clone()));
    (routes, default)
}

/// The Claude flags a route gets: `--model`, `--effort`, `--agent` (those present).
#[must_use]
pub fn route_flags(route: &Route) -> Vec<String> {
    route_flags_for(route, false)
}

/// [`route_flags`] for a run on a free model route: the pack's `--model` names a Claude model, which a
/// proxy cannot serve, so it is left out (`free`); effort and agent still apply.
#[must_use]
pub fn route_flags_for(route: &Route, free: bool) -> Vec<String> {
    let mut a = Vec::new();
    for (flag, val) in [
        ("--model", &route.model),
        ("--effort", &route.effort),
        ("--agent", &route.agent),
    ] {
        if flag == "--model" && free {
            continue;
        }
        if let Some(v) = val {
            a.extend([flag.to_owned(), v.clone()]);
        }
    }
    a
}

/// The route a request names: no id gives the pack's default (none when it has no routes); an id the
/// pack does not declare is an error.
///
/// # Errors
/// [`PackError::UnknownRoute`] for an unknown id.
pub fn find_route<'a>(meta: &'a PackMeta, id: Option<&str>) -> Result<Option<&'a Route>, PackError> {
    match id {
        Some(id) => meta
            .routes
            .iter()
            .find(|r| r.id == id)
            .map(Some)
            .ok_or_else(|| PackError::UnknownRoute(id.to_owned())),
        None => Ok(meta
            .default_route
            .as_deref()
            .and_then(|d| meta.routes.iter().find(|r| r.id == d))),
    }
}

fn read_pack(kun: &Path, dept: &str) -> RawPack {
    fs::read_to_string(kun.join("departments").join(dept).join("pack.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Builds (or refreshes) `<machine>/<dept>/` from `<kun>/departments/<dept>/`.
///
/// # Errors
/// On a bad name, a department without a `prompt.md`, or a filesystem failure.
pub fn ensure_in(kun: &Path, machine: &Path, dept: &str) -> Result<PathBuf, PackError> {
    if !valid_name(dept) {
        return Err(PackError::BadName);
    }
    let prompt_path = kun.join("departments").join(dept).join("prompt.md");
    let prompt = fs::read_to_string(&prompt_path)
        .map_err(|_| PackError::Missing(dept.to_owned(), prompt_path.display().to_string()))?;
    let dir = machine.join(dept);
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;

    let rules = kun.join("CLAUDE.md");
    let body = prompt.trim_end();
    write_if_changed(
        &dir.join("CLAUDE.md"),
        &format!(
            "{body}\n\n---\n\n@{}\n",
            rules.display().to_string().replace('\\', "/")
        ),
    )?;
    write_if_changed(
        &dir.join("AGENTS.md"),
        &format!(
            "{body}\n\n---\n\nCompany rules: read {} before you start.\n",
            rules.display()
        ),
    )?;

    let pack = read_pack(kun, dept);
    let wanted = item_names(&pack.items);
    link::link_skills(
        &kun.join(".claude/skills"),
        &dir.join(".claude/skills"),
        &wanted,
    )?;
    let agents = kun.join(".agents/skills");
    if agents.is_dir() {
        link::link_skills(&agents, &dir.join(".agents/skills"), &wanted)?;
    }
    link::link_agents(
        &kun.join(".claude/agents"),
        &dir.join(".claude/agents"),
        &agent_names(&pack.items),
    )?;
    Ok(dir)
}

/// The department's working folder, built on demand. Chat runs every job in it.
///
/// # Errors
/// See [`ensure_in`].
pub fn workdir(dept: &str) -> Result<PathBuf, PackError> {
    ensure_in(&kun_dir()?, &machine_dir()?, dept)
}

/// A pack's titles, quick actions and skill names: what profiles and the UI read.
#[derive(Debug, Clone, Default)]
pub struct PackMeta {
    pub title_ar: Option<String>,
    pub title_en: Option<String>,
    pub quick_actions: Vec<QuickAction>,
    pub skills: Vec<String>,
    pub default_driver: Option<DriverKind>,
    /// Home card ids, in the pack's order.
    pub cards: Vec<String>,
    /// The repos this department works in, in the pack's order.
    pub repos: Vec<String>,
    /// Extra folders the pack asks for, as written (`~/media`); resolve with [`pack_dirs`].
    pub dirs: Vec<String>,
    /// The turn timeout the pack asks for, clamped to [`MIN_TIMEOUT_SECS`]..=[`MAX_TIMEOUT_SECS`].
    pub timeout_secs: Option<u32>,
    /// The health check and fix the pack declares, if any (validated when run, see `health.rs`).
    pub health: Option<health::HealthSpec>,
    /// Model, effort and agent the pack picks for its Claude turns.
    pub run: Option<RunMeta>,
    /// Named run routes (validated), at most [`MAX_ROUTES`]; derived from `run` when none are declared.
    pub routes: Vec<Route>,
    /// The route a request without one uses: `defaultRoute` when it exists, else the first.
    pub default_route: Option<String>,
    /// The lead agent, the produce-style line and the guard agent (names, validated).
    pub lead: Option<String>,
    pub line: Option<String>,
    pub guard: Option<String>,
    pub keywords: Keywords,
    /// Rules every prompt of the department carries.
    pub constraints: Vec<String>,
    /// Things refine reads first; each is one thing to read.
    pub gather: Vec<String>,
}

/// The most entries of a pack's `constraints`, `gather` and each `keywords` list.
pub const MAX_LIST: usize = 24;
/// The longest `constraints` / `gather` entry, in characters; a longer one is dropped.
pub const MAX_LINE_CHARS: usize = 600;

/// The non-blank strings of a list, trimmed, at most [`MAX_LIST`], each at most `max` characters.
fn clean_lines(raw: &[Value], max: usize) -> Vec<String> {
    raw.iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.chars().count() <= max)
        .take(MAX_LIST)
        .map(str::to_owned)
        .collect()
}

fn parse_keywords(raw: Option<&Value>) -> Keywords {
    let list = |key: &str| {
        raw.and_then(|v| v.get(key))
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |a| clean_lines(a, 64))
    };
    Keywords { en: list("en"), ar: list("ar") }
}

/// The shortest and longest turn timeout a pack may ask for.
pub const MIN_TIMEOUT_SECS: u32 = 60;
pub const MAX_TIMEOUT_SECS: u32 = 14_400;

/// Expands a leading `~/`; anything else must already be absolute.
fn expand_home(raw: &str, home: &Path) -> Option<PathBuf> {
    let raw = raw.trim();
    let path = match raw.strip_prefix("~/") {
        Some(rest) if !rest.trim_matches('/').is_empty() => home.join(rest),
        Some(_) => return None,
        None => PathBuf::from(raw),
    };
    let clean = path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
    clean.then_some(path)
}

/// One pack folder, validated against `home`: absolute, no `..`, symlink-resolved and strictly
/// under `home`. A missing folder is created only when its parent already exists under `home`.
fn resolve_dir(raw: &str, home: &Path) -> Option<PathBuf> {
    let path = expand_home(raw, home)?;
    let under = |p: &Path| p != home && p.starts_with(home);
    if let Ok(real) = path.canonicalize() {
        return (under(&real) && real.is_dir()).then_some(real);
    }
    let parent = path.parent()?.canonicalize().ok()?;
    if !(parent == home || under(&parent)) {
        return None;
    }
    fs::create_dir_all(&path).ok()?;
    let real = path.canonicalize().ok()?;
    under(&real).then_some(real)
}

/// The pack's extra folders resolved under `home` (canonical). Bad entries are skipped with a warning.
#[must_use]
pub fn pack_dirs_in(meta: &PackMeta, home: &Path) -> Vec<PathBuf> {
    let Ok(home) = home.canonicalize() else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = Vec::new();
    for raw in &meta.dirs {
        match resolve_dir(raw, &home) {
            Some(dir) if !out.contains(&dir) => out.push(dir),
            Some(_) => {}
            None => tracing::warn!("pack dir `{raw}` skipped: not a folder under the home folder"),
        }
    }
    out
}

/// [`pack_dirs_in`] for the real home folder.
#[must_use]
pub fn pack_dirs(meta: &PackMeta) -> Vec<PathBuf> {
    dirs::home_dir().map_or_else(Vec::new, |h| pack_dirs_in(meta, &h))
}

/// Reads `departments/<dept>/pack.json`. A missing file gives an empty meta.
///
/// # Errors
/// On a bad name.
pub fn meta_in(kun: &Path, dept: &str) -> Result<PackMeta, PackError> {
    if !valid_name(dept) {
        return Err(PackError::BadName);
    }
    let raw = read_pack(kun, dept);
    let pick = |key: &str| match &raw.title {
        Value::Object(o) => o.get(key).and_then(Value::as_str).map(str::to_owned),
        _ => None,
    };
    let plain = match &raw.title {
        Value::String(s) => Some(s.clone()),
        _ => None,
    };
    let run = parse_run(raw.run.as_ref());
    let lanes = read_lanes(kun);
    let (routes, default_route) =
        parse_routes(&raw.routes, raw.default_route.as_deref(), run.as_ref(), &lanes);
    let name = |v: &Option<String>| v.clone().filter(|n| valid_name(n));
    Ok(PackMeta {
        routes,
        default_route,
        lead: name(&raw.lead),
        line: name(&raw.line),
        guard: name(&raw.guard),
        keywords: parse_keywords(raw.keywords.as_ref()),
        constraints: clean_lines(&raw.constraints, MAX_LINE_CHARS),
        gather: clean_lines(&raw.gather, MAX_LINE_CHARS),
        title_ar: raw.title_ar.clone().or_else(|| pick("ar")),
        title_en: pick("en").or(plain),
        skills: item_names(&raw.items),
        default_driver: raw.default_driver,
        cards: raw.cards.iter().filter(|c| valid_name(c)).cloned().collect(),
        repos: raw.repos.iter().filter(|r| valid_name(r)).cloned().collect(),
        dirs: raw.dirs.clone(),
        health: raw.health.clone(),
        run,
        timeout_secs: raw
            .timeout_secs
            .map(|t| t.clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)),
        quick_actions: raw
            .quick_actions
            .into_iter()
            .filter(|q| !q.prompt.trim().is_empty())
            .map(|q| QuickAction {
                id: q.id.filter(|i| station::valid_formula_id(i)),
                label_ar: q.label_ar,
                label_en: q.label_en,
                prompt: q.prompt,
                description_en: q.description_en,
                description_ar: q.description_ar,
                repos: q.repos.into_iter().filter(|r| valid_name(r)).collect(),
                route: q.route.filter(|r| valid_route_id(r)),
            })
            .collect(),
    })
}

/// A department's pack as the UI needs it. A missing `pack.json` gives an empty pack.
///
/// # Errors
/// On a bad name.
pub fn info_in(kun: &Path, dept: &str) -> Result<PackInfo, PackError> {
    let meta = meta_in(kun, dept)?;
    let raw = read_pack(kun, dept);
    let formula = parse_formula(raw.formula.as_ref());
    let fallback = raw.name.unwrap_or_else(|| dept.to_owned());
    Ok(PackInfo {
        department: dept.to_owned(),
        title_ar: meta.title_ar.unwrap_or_else(|| fallback.clone()),
        title_en: meta.title_en.unwrap_or(fallback),
        default_driver: meta.default_driver,
        quick_actions: meta.quick_actions,
        formula,
        lead: meta.lead,
        line: meta.line,
        guard: meta.guard,
        keywords: meta.keywords,
        constraints: meta.constraints,
        gather: meta.gather,
        routes: meta.routes,
        default_route: meta.default_route,
    })
}

/// Departments that ship a pack in this kun checkout, sorted. A missing `departments/` is empty.
#[must_use]
pub fn departments_in(kun: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(kun.join("departments"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("prompt.md").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| valid_name(n))
        .collect();
    names.sort();
    names
}

/// `departments` — which department packs exist.
#[tauri::command]
fn departments() -> Result<Vec<String>, PackError> {
    Ok(departments_in(&kun_dir()?))
}

/// `info` — one department's title, fallback quick actions, formula and routes.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn info(department: String) -> Result<PackInfo, PackError> {
    info_in(&kun_dir()?, &department)
}

/// `board` — the station's departure board: at most three options, never an error.
#[tauri::command]
async fn board(department: String, repo: Option<String>, health_level: Option<String>) -> station::Board {
    station::board(&department, repo.as_deref(), health_level.as_deref()).await
}

/// `fill` — one formula filled by the station's script, nothing sent anywhere.
#[tauri::command]
async fn fill(
    department: String,
    id: String,
    repo: Option<String>,
    set: Option<BTreeMap<String, String>>,
) -> Result<station::Filled, PackError> {
    station::fill(&department, &id, repo.as_deref(), &set.unwrap_or_default()).await
}

/// `match_text` — the station's switch: typed text to a formula id, or none.
#[tauri::command]
async fn match_text(department: String, text: String, repo: Option<String>) -> station::Match {
    station::match_text(&department, &text, repo.as_deref()).await
}

/// One formula for the slash menu: its words and icon, no body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct FormulaMeta {
    /// The formula id, e.g. `video.film.plan`.
    pub id: String,
    pub label_en: String,
    pub label_ar: String,
    /// `when.intent`: one line on when the formula is the right one.
    pub intent: String,
    /// A lucide icon name, when the header names one.
    pub icon: Option<String>,
}

/// The header of a formula file: the JSON between the leading `---` fences.
fn formula_header(raw: &str) -> Option<Value> {
    let rest = raw.strip_prefix("---")?.trim_start_matches(['\r', '\n']);
    let end = rest.find("\n---")?;
    serde_json::from_str(&rest[..end]).ok()
}

/// A department's formulas, read from `departments/<dept>/formulas/*.md`, ordered by id. Files that
/// do not parse, or whose id is not a valid name, are skipped; a missing folder is an empty list.
///
/// # Errors
/// [`PackError::BadName`] when the department name is invalid.
pub fn formulas_in(kun: &Path, dept: &str) -> Result<Vec<FormulaMeta>, PackError> {
    if !valid_name(dept) {
        return Err(PackError::BadName);
    }
    let Ok(rd) = fs::read_dir(kun.join("departments").join(dept).join("formulas")) else {
        return Ok(Vec::new());
    };
    let text = |v: &Value| v.as_str().unwrap_or_default().trim().to_owned();
    let mut out: Vec<FormulaMeta> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|raw| formula_header(&raw))
        .filter_map(|h| {
            let id = text(&h["id"]);
            valid_name(&id).then(|| FormulaMeta {
                label_en: Some(text(&h["label"]["en"])).filter(|s| !s.is_empty()).unwrap_or_else(|| id.clone()),
                label_ar: text(&h["label"]["ar"]),
                intent: text(&h["when"]["intent"]),
                icon: Some(text(&h["icon"])).filter(|s| !s.is_empty()),
                id,
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// `formulas` — the department's formulas for the slash menu; read-only.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn formulas(department: String) -> Result<Vec<FormulaMeta>, PackError> {
    formulas_in(&kun_dir()?, &department)
}

/// `station_log` — appends a tap or a dismissal to `~/.kun/machine/station-log.jsonl`.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn station_log(
    department: String,
    repo: Option<String>,
    id: String,
    event: station::LogEvent,
) -> Result<(), PackError> {
    station::log_event(&department, repo.as_deref(), &id, event)
}

/// `health` — runs the department's declared check and returns the light.
#[tauri::command]
async fn health(department: String) -> Result<health::Health, PackError> {
    run_health(&department, false).await
}

/// `health_fix` — runs the department's declared fix, then the check again.
#[tauri::command]
async fn health_fix(department: String) -> Result<health::Health, PackError> {
    run_health(&department, true).await
}

async fn run_health(department: &str, fix: bool) -> Result<health::Health, PackError> {
    let kun = kun_dir()?;
    let home = dirs::home_dir().ok_or(PackError::NoHome)?;
    let meta = meta_in(&kun, department)?;
    Ok(health::evaluate(meta.health.as_ref(), &home, &kun, fix).await)
}

/// The base folder for profile workspaces: `KUN_HOME/Kun` when `KUN_HOME` is set, else `~/Kun`.
///
/// # Errors
/// When there is no home folder and no override.
pub fn workspace_root() -> Result<PathBuf, PackError> {
    match std::env::var_os("KUN_HOME") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p).join("Kun")),
        _ => dirs::home_dir()
            .map(|h| h.join("Kun"))
            .ok_or(PackError::NoHome),
    }
}

/// The profile's workspace (`<root>/<login>/`), built on demand.
///
/// # Errors
/// See [`ensure_workspace_in`].
pub fn workspace(profile: &crate::profile::MachineProfile) -> Result<Workspace, PackError> {
    let repos = crate::repos::repos_root().map_err(|e| PackError::Profile(e.to_string()))?;
    ensure_workspace_with(&kun_dir()?, &workspace_root()?, &repos, profile)
}

/// `prepare` — builds the working folder and returns its path. A signed-in profile gets its
/// workspace, anyone else the department folder.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn prepare(department: String) -> Result<String, PackError> {
    let resolved = crate::profile::current().map_err(|e| PackError::Profile(e.to_string()))?;
    if resolved.explicit {
        return workspace(&resolved.profile).map(|w| w.dir.display().to_string());
    }
    workdir(&department).map(|p| p.display().to_string())
}

/// `sync_repos` — clones the profile's repos into `~/Kun/repos/` or fast-forwards them,
/// streaming one event per step. Never pushes.
#[tauri::command]
async fn sync_repos(channel: Channel<SyncEvent>) -> Result<(), PackError> {
    let resolved = crate::profile::current().map_err(|e| PackError::Profile(e.to_string()))?;
    let root = crate::repos::repos_root().map_err(|e| PackError::Profile(e.to_string()))?;
    crate::repos::sync_all(
        &root,
        &resolved.profile.repos,
        crate::repos::GITHUB_BASE,
        |e| {
            let _ = channel.send(e);
        },
    )
    .await
    .map_err(|e| match e {
        crate::repos::RepoError::BadName(n) => PackError::BadRepo(n),
        other => PackError::Profile(other.to_string()),
    })
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("pack")
        .invoke_handler(tauri::generate_handler![
            departments,
            info,
            prepare,
            sync_repos,
            health,
            health_fix,
            board,
            fill,
            match_text,
            formulas,
            station_log
        ])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
