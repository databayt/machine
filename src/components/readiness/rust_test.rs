use std::{path::Path, time::Duration};

use tokio::sync::Notify;

use super::{
    agents, catalog,
    catalog::Scope,
    fix, heal,
    model::{
        Catalog, FixKind, Mode, ReadinessError, ReadinessState, RunEvent, StepStatus,
        UninstallKind, UninstallOutcome,
    },
    run, uninstall,
};

const CATALOG: &str = include_str!("fixtures/readiness.json");
const STATE: &str = include_str!("fixtures/state.json");
const STREAM: &str = include_str!("fixtures/stream.ndjson");

fn catalog_fixture() -> Catalog {
    catalog::parse(CATALOG).unwrap()
}

fn state_fixture() -> ReadinessState {
    serde_json::from_str(STATE).unwrap()
}

fn scope(departments: &[&str], role: &str) -> Scope {
    Scope {
        os: "mac".into(),
        departments: departments.iter().map(|d| (*d).to_owned()).collect(),
        role: role.into(),
    }
}

fn ids(steps: &[super::model::Step]) -> Vec<&str> {
    steps.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn catalog_filters_by_os_department_and_role() {
    let c = catalog_fixture();
    let member = catalog::ordered(&c, &scope(&[], "member"));
    assert_eq!(
        ids(&member),
        [
            "github.org",
            "brew",
            "node",
            "claude.cli",
            "engine.notify",
            "engine.health"
        ]
    );
    let linux = Scope {
        os: "linux".into(),
        ..scope(&[], "member")
    };
    // brew is mac only; claude lists linux too.
    assert!(!ids(&catalog::ordered(&c, &linux)).contains(&"brew"));
    assert!(ids(&catalog::ordered(&c, &linux)).contains(&"claude.cli"));
}

#[test]
fn catalog_orders_by_stage_then_after() {
    let c = catalog_fixture();
    let engineer = catalog::ordered(&c, &scope(&["engineering", "media"], "engineer"));
    assert_eq!(
        ids(&engineer),
        [
            "github.org",
            "brew",
            "node",
            "tool.blender",
            "claude.cli",
            "repo.hogwarts",
            "engine.notify",
            "engine.health"
        ]
    );
}

#[test]
fn an_early_step_waits_for_a_later_stage_it_comes_after() {
    let c = catalog::parse(
        r#"{"version":1,"stages":["a","b"],"steps":[
            {"id":"x","stage":"a","after":["y"]},
            {"id":"y","stage":"b"},
            {"id":"z","stage":"a"}]}"#,
    )
    .unwrap();
    assert_eq!(
        ids(&catalog::ordered(&c, &scope(&[], "member"))),
        ["z", "y", "x"]
    );
}

#[test]
fn a_cycle_or_a_missing_after_cannot_stall_the_list() {
    let c = catalog::parse(
        r#"{"stages":["a"],"steps":[
            {"id":"p","stage":"a","after":["q"]},
            {"id":"q","stage":"a","after":["p"]},
            {"id":"r","stage":"a","after":["nowhere"]}]}"#,
    )
    .unwrap();
    assert_eq!(catalog::ordered(&c, &scope(&[], "member")).len(), 3);
}

#[test]
fn stages_may_be_ids_or_objects_and_wildcards_match_everything() {
    let c = catalog::parse(
        r#"{"version":"v2","stages":["a",{"id":"b","title":{"ar":"ب","en":"B"}}],
            "steps":[{"id":"s","stage":"b","os":["*"],"departments":["*"],"roles":["*"]}]}"#,
    )
    .unwrap();
    assert_eq!(c.stages[1].title.as_ref().unwrap().en, "B");
    assert_eq!(catalog::ordered(&c, &scope(&["sales"], "member")).len(), 1);
}

#[test]
fn a_damaged_catalog_is_an_error_not_a_panic() {
    assert!(matches!(
        catalog::parse("{nope"),
        Err(ReadinessError::BadCatalog(_))
    ));
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        catalog::load(dir.path()),
        Err(ReadinessError::NoCatalog(_))
    ));
}

#[test]
fn the_view_merges_state_into_the_ordered_steps() {
    let c = catalog_fixture();
    let s = state_fixture();
    let v = catalog::view(Some(&c), &scope(&[], "member"), Some(&s));
    assert!(v.catalog_found && v.has_state && !v.complete);
    assert!((v.percent - 62.5).abs() < f64::EPSILON);
    let claude = v.steps.iter().find(|x| x.step.id == "claude.cli").unwrap();
    assert_eq!(claude.status, StepStatus::Fail);
    assert_eq!(claude.detail, "claude not found");
    let notify = v
        .steps
        .iter()
        .find(|x| x.step.id == "engine.notify")
        .unwrap();
    assert_eq!(notify.status, StepStatus::Pending);
    let none = catalog::view(None, &scope(&[], "member"), None);
    assert!(!none.catalog_found && !none.has_state && !none.complete && none.steps.is_empty());
}

#[test]
fn the_heartbeat_carries_a_whole_percent() {
    let beat = catalog::beat(&state_fixture(), None, &scope(&[], "member"), 0);
    assert_eq!(beat.percent, 63);
    assert_eq!(beat.failing, ["claude.cli", "engine.health"]);
    let json = serde_json::to_value(&beat).unwrap();
    assert_eq!(json["catalogVersion"], 3);
}

#[test]
fn every_runner_line_parses_and_odd_lines_become_logs() {
    let events: Vec<RunEvent> = STREAM.lines().filter_map(run::parse_line).collect();
    assert_eq!(events.len(), 10);
    assert!(matches!(&events[0], RunEvent::Start { mode, total: 6, .. } if mode == "complete"));
    assert!(matches!(
        &events[3],
        RunEvent::Step { id, status: StepStatus::Fixing, .. } if id == "claude.cli"
    ));
    assert!(matches!(&events[5], RunEvent::Pass { n: 1 }));
    assert!(matches!(&events[7], RunEvent::Log { line } if line.contains("future-event")));
    assert!(matches!(&events[8], RunEvent::Log { line } if line == "not json at all"));
    assert!(matches!(
        &events[9],
        RunEvent::Done { passed: 5, applicable: 6, failing, human, .. }
            if failing == &["engine.health"] && human == &["github.org"]
    ));
    assert!(run::parse_line("   ").is_none());
    // A status a newer runner invents does not break the stream.
    assert!(matches!(
        run::parse_line(r#"{"type":"step","id":"a","status":"quarantined"}"#),
        Some(RunEvent::Step {
            status: StepStatus::Unknown,
            ..
        })
    ));
}

#[test]
fn runner_args_are_a_vector_and_reject_bad_ids() {
    let script = Path::new("/kun/.claude/skills/provision/scripts/provision.mjs");
    let args = run::build_args(
        script,
        Mode::Complete,
        &["gh.installed".into(), "claude.cli".into()],
        &scope(&["sales", "video"], "engineer"),
    )
    .unwrap();
    assert_eq!(
        args,
        [
            script.to_str().unwrap(),
            "--complete",
            "--only",
            "gh.installed,claude.cli",
            "--departments",
            "sales,video",
            "--role",
            "engineer",
            "--json"
        ]
    );
    for bad in ["--role", "a b", "a;b", "", "$(x)", "-x", "a,b"] {
        assert!(
            matches!(
                run::build_args(script, Mode::Check, &[bad.into()], &scope(&[], "member")),
                Err(ReadinessError::BadId(_))
            ),
            "{bad:?} must be refused"
        );
    }
    let plain = run::build_args(script, Mode::Maintain, &[], &scope(&[], "member")).unwrap();
    assert!(!plain.contains(&"--only".to_owned()) && !plain.contains(&"--departments".to_owned()));
}

#[tokio::test]
async fn a_real_process_streams_events_and_summarizes_done() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../src/components/readiness/fixtures/stream.ndjson"
    );
    let mut seen = Vec::new();
    let summary = run::stream(
        Path::new("/bin/cat"),
        &[fixture.to_owned()],
        Path::new("/"),
        Duration::from_secs(10),
        &Notify::new(),
        &mut |e| seen.push(e),
    )
    .await
    .unwrap();
    assert_eq!(seen.len(), 10);
    assert_eq!(summary.passed, 5);
    assert_eq!(summary.failing, ["engine.health"]);
    assert!(!summary.cancelled);
}

#[tokio::test]
async fn a_failing_process_without_done_is_an_error_event() {
    let mut seen = Vec::new();
    let result = run::stream(
        Path::new("/bin/sh"),
        &["-c".into(), "echo boom >&2; exit 3".into()],
        Path::new("/"),
        Duration::from_secs(10),
        &Notify::new(),
        &mut |e| seen.push(e),
    )
    .await;
    assert!(
        matches!(result, Err(ReadinessError::Failed(m)) if m.contains('3') && m.contains("boom"))
    );
    assert!(matches!(seen.last(), Some(RunEvent::Error { .. })));
}

#[tokio::test]
async fn cancel_kills_the_child_quickly() {
    let cancel = std::sync::Arc::new(Notify::new());
    let trigger = std::sync::Arc::clone(&cancel);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.notify_waiters();
    });
    let started = std::time::Instant::now();
    let summary = run::stream(
        Path::new("/bin/sleep"),
        &["30".into()],
        Path::new("/"),
        Duration::from_secs(60),
        &cancel,
        &mut |_| {},
    )
    .await
    .unwrap();
    assert!(summary.cancelled);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn a_timeout_is_an_error() {
    let result = run::stream(
        Path::new("/bin/sleep"),
        &["30".into()],
        Path::new("/"),
        Duration::from_millis(150),
        &Notify::new(),
        &mut |_| {},
    )
    .await;
    assert!(matches!(result, Err(ReadinessError::Failed(_))));
}

#[test]
fn one_run_at_a_time() {
    let first = run::acquire().unwrap();
    assert!(matches!(run::acquire(), Err(ReadinessError::Busy)));
    assert!(run::cancel());
    drop(first);
    assert!(run::acquire().is_ok());
}

fn step(id: &str, fix: &str) -> super::model::Step {
    serde_json::from_str(&format!(r#"{{"id":"{id}","stage":"s","fix":{fix}}}"#)).unwrap()
}

#[test]
fn fix_kinds_become_actions() {
    let kun = Path::new("/Users/x/kun");
    assert_eq!(
        fix::plan(&step("a", r#"{"kind":"auto"}"#), kun).unwrap(),
        fix::Action::Provision("a".into())
    );
    assert_eq!(
        fix::plan(
            &step("a", r#"{"kind":"terminal","cmd":"brew install jq"}"#),
            kun
        )
        .unwrap(),
        fix::Action::Terminal("cd '/Users/x/kun' && brew install jq".into())
    );
    assert_eq!(
        fix::plan(
            &step("a", r#"{"kind":"link","url":"https://brew.sh"}"#),
            kun
        )
        .unwrap(),
        fix::Action::Link("https://brew.sh".into())
    );
    assert_eq!(
        fix::plan(&step("a", r#"{"kind":"request","message":"add me"}"#), kun).unwrap(),
        fix::Action::Request("[readiness a] add me".into())
    );
    for bad in [
        r#"{"kind":"link","url":"javascript:alert(1)"}"#,
        r#"{"kind":"link"}"#,
        r#"{"kind":"terminal"}"#,
        r#"{"kind":"request"}"#,
    ] {
        assert!(fix::plan(&step("a", bad), kun).is_err(), "{bad}");
    }
    let nofix: super::model::Step = serde_json::from_str(r#"{"id":"n","stage":"s"}"#).unwrap();
    assert!(matches!(
        fix::plan(&nofix, kun),
        Err(ReadinessError::NoFix(_))
    ));
}

#[test]
fn applescript_text_is_escaped() {
    assert_eq!(
        fix::applescript_escape(r#"say "hi" \ done"#),
        r#"say \"hi\" \\ done"#
    );
    let args = fix::osascript_args(r#"echo "a\b""#);
    assert_eq!(args[0], "-e");
    assert_eq!(
        args[1],
        r#"tell application "Terminal" to do script "echo \"a\\b\"""#
    );
    assert_eq!(fix::shell_quote("it's"), r"'it'\''s'");
}

#[test]
fn a_placeholder_command_is_shown_not_run() {
    let f: super::model::Fix = serde_json::from_str(
        r#"{"kind":"terminal","cmd":"bash .claude/scripts/secrets.sh <GIST_ID>","message":"Ask for the id"}"#,
    )
    .unwrap();
    let line = fix::terminal_line(Path::new("/k"), &f).unwrap();
    assert!(line.starts_with("cd '/k' && printf"));
    assert!(line.contains("'Ask for the id'"));
    assert!(!fix::has_placeholder("a < b"));
    assert_eq!(f.kind, FixKind::Terminal);
}

#[test]
fn the_heal_attempt_cap_is_three_per_day() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("heal-attempts.json");
    let t0 = 1_000_000;
    assert!(heal::under_cap(&heal::load(&path), "a", t0));
    for i in 0..3 {
        heal::record(&path, "a", t0 + i).unwrap();
    }
    let ledger = heal::load(&path);
    assert!(
        !heal::under_cap(&ledger, "a", t0 + 100),
        "third attempt used up the cap"
    );
    assert!(
        heal::under_cap(&ledger, "b", t0 + 100),
        "another step is unaffected"
    );
    assert!(
        heal::under_cap(&ledger, "a", t0 + heal::WINDOW_SECS + 3),
        "the window rolls over"
    );
    // Old attempts are pruned when a new one is recorded.
    heal::record(&path, "a", t0 + heal::WINDOW_SECS + 10).unwrap();
    assert_eq!(heal::load(&path)["a"].len(), 1);
}

#[test]
fn only_failing_auto_steps_under_the_cap_are_candidates() {
    let c = catalog_fixture();
    let mut s = state_fixture();
    s.failing = vec![
        "claude.cli".into(),
        "engine.health".into(),
        "github.org".into(),
        "node".into(),
    ];
    let sc = scope(&[], "member");
    let mut ledger = heal::Ledger::new();
    // node is terminal, github.org is a human request, claude.cli and engine.health are auto.
    assert_eq!(
        heal::candidates(&c, &sc, &s, &ledger, 10),
        ["claude.cli", "engine.health"]
    );
    ledger.insert("claude.cli".into(), vec![1, 2, 3]);
    assert_eq!(
        heal::candidates(&c, &sc, &s, &ledger, 10),
        ["engine.health"]
    );
}

#[test]
fn agents_and_tools_merge_with_their_catalog_steps() {
    use crate::{
        driver::{DriverKind, DriverStatus},
        tools::ToolStatus,
    };
    let c = catalog_fixture();
    let s = state_fixture();
    let drivers = [
        DriverStatus {
            kind: DriverKind::Claude,
            installed: false,
            path: None,
        },
        DriverStatus {
            kind: DriverKind::Codex,
            installed: true,
            path: Some("/bin/codex".into()),
        },
    ];
    let tools = [ToolStatus {
        id: "blender".into(),
        label: "Blender".into(),
        installed: false,
        path: None,
        version: None,
        needs_key: false,
    }];
    let rows = agents::merge(&drivers, &tools, Some(&c), Some(&s));
    let claude = rows.iter().find(|r| r.id == "claude").unwrap();
    assert_eq!(claude.step_id.as_deref(), Some("claude.cli"));
    assert_eq!(claude.status, Some(StepStatus::Fail));
    assert!(claude.can_fix && claude.can_update);
    let codex = rows.iter().find(|r| r.id == "codex").unwrap();
    assert!(codex.installed && codex.step_id.is_none() && !codex.can_fix);
    let blender = rows.iter().find(|r| r.id == "blender").unwrap();
    assert_eq!(blender.step_id.as_deref(), Some("tool.blender"));
    // Without a catalog the rows still list what is installed.
    assert_eq!(agents::merge(&drivers, &tools, None, None).len(), 3);
}

fn step_event(id: &str, status: &str, detail: &str) -> RunEvent {
    run::parse_line(&format!(
        r#"{{"type":"step","id":"{id}","stage":"s","status":"{status}","detail":"{detail}"}}"#
    ))
    .unwrap()
}

#[test]
fn the_uninstall_mode_builds_uninstall_and_only() {
    let script = Path::new("/kun/.claude/skills/provision/scripts/provision.mjs");
    let args = run::build_args(
        script,
        Mode::Uninstall,
        &["tool.blender".into(), "node".into()],
        &scope(&["media"], "engineer"),
    )
    .unwrap();
    assert_eq!(
        args,
        [
            script.to_str().unwrap(),
            "--uninstall",
            "--only",
            "tool.blender,node",
            "--departments",
            "media",
            "--role",
            "engineer",
            "--json"
        ]
    );
    assert_eq!(
        serde_json::from_str::<Mode>(r#""uninstall""#).unwrap(),
        Mode::Uninstall
    );
}

#[test]
fn a_catalog_step_may_or_may_not_carry_an_uninstall() {
    let c = catalog_fixture();
    let get = |id: &str| c.steps.iter().find(|s| s.id == id).unwrap();
    let auto = get("claude.cli").uninstall.as_ref().unwrap();
    assert_eq!(auto.kind, UninstallKind::Auto);
    assert!(!auto.keystone && auto.cmd.starts_with("npm rm"));
    let term = get("tool.blender").uninstall.as_ref().unwrap();
    assert_eq!(term.kind, UninstallKind::Terminal);
    assert!(!term.keystone);
    assert!(get("brew").uninstall.as_ref().unwrap().keystone);
    assert!(get("node").uninstall.is_none());
    // A catalog from before version 2 has no such field anywhere and still loads.
    let old = catalog::parse(r#"{"version":1,"stages":["a"],"steps":[{"id":"x","stage":"a"}]}"#)
        .unwrap();
    assert!(old.steps[0].uninstall.is_none());
}

#[test]
fn removing_and_removed_parse() {
    assert!(matches!(
        run::parse_line(r#"{"type":"step","id":"a","status":"removing","detail":"npm rm"}"#),
        Some(RunEvent::Step {
            status: StepStatus::Removing,
            ..
        })
    ));
    assert!(matches!(
        run::parse_line(r#"{"type":"step","id":"a","status":"removed"}"#),
        Some(RunEvent::Step {
            status: StepStatus::Removed,
            ..
        })
    ));
    assert_eq!(
        serde_json::to_string(&StepStatus::Removed).unwrap(),
        r#""removed""#
    );
}

#[test]
fn removed_steps_reach_the_view() {
    let v = catalog::view(
        Some(&catalog_fixture()),
        &scope(&[], "member"),
        Some(&state_fixture()),
    );
    assert_eq!(v.removed, ["tool.blender"]);
    let none = catalog::view(None, &scope(&[], "member"), None);
    assert!(none.removed.is_empty());
    // A state file from before `removed` existed still loads.
    let old: ReadinessState = serde_json::from_str(r#"{"percent":10}"#).unwrap();
    assert!(old.removed.is_empty());
}

#[test]
fn the_outcome_sorts_events_into_removed_blocked_failed_handoff() {
    let c = catalog_fixture();
    let requested: Vec<String> = ["claude.cli", "node", "tool.blender", "brew", "github.org"]
        .map(String::from)
        .into();
    let events = vec![
        step_event("claude.cli", "removing", "npm rm"),
        step_event("claude.cli", "removed", ""),
        step_event("node", "blocked", "needed by: claude.cli"),
        // The event text names another command; the catalog's wins.
        step_event("tool.blender", "human", "rm -rf nothing"),
        step_event("brew", "human", "bash uninstall-brew.sh"),
        step_event("github.org", "fail", "no uninstaller"),
        // Not requested: ignored.
        step_event("engine.health", "removed", ""),
        run::parse_line(r#"{"type":"pass","n":1}"#).unwrap(),
        // The final check pass reports the removed step again; it must not undo the outcome.
        step_event("claude.cli", "skip", "removed by you"),
        step_event("node", "pass", "v24"),
    ];
    let out = uninstall::outcome(&requested, &events, Some(&c), false);
    assert_eq!(out.removed, ["claude.cli"]);
    assert_eq!(out.blocked.len(), 1);
    assert_eq!(out.blocked[0].id, "node");
    assert_eq!(out.blocked[0].detail, "needed by: claude.cli");
    assert_eq!(out.failed.len(), 1);
    assert_eq!(out.failed[0].detail, "no uninstaller");
    assert_eq!(out.handoff.len(), 2);
    assert_eq!(out.handoff[0].id, "tool.blender");
    assert_eq!(out.handoff[0].cmd, "brew uninstall --cask blender");
    assert!(!out.handoff[0].keystone);
    assert_eq!(out.handoff[1].cmd, "bash uninstall-brew.sh");
    assert!(out.handoff[1].keystone);
    assert!(!out.cancelled);
    // A cancelled run with nothing seen is an empty outcome.
    let empty = uninstall::outcome(&requested, &[], None, true);
    assert_eq!(
        empty,
        UninstallOutcome {
            cancelled: true,
            ..UninstallOutcome::default()
        }
    );
}

#[test]
fn the_uninstall_terminal_line_comes_from_the_catalog() {
    let kun = Path::new("/Users/x/kun");
    let c = catalog_fixture();
    let get = |id: &str| c.steps.iter().find(|s| s.id == id).unwrap();
    assert_eq!(
        fix::uninstall_line(kun, get("tool.blender")).unwrap(),
        "cd '/Users/x/kun' && brew uninstall --cask blender"
    );
    // A keystone is handed over whatever its kind.
    assert!(fix::uninstall_line(kun, get("brew")).is_ok());
    // An auto uninstall is the runner's; a step without one has nothing to open.
    assert!(matches!(
        fix::uninstall_line(kun, get("claude.cli")),
        Err(ReadinessError::NoUninstall(_))
    ));
    assert!(matches!(
        fix::uninstall_line(kun, get("node")),
        Err(ReadinessError::NoUninstall(_))
    ));
    let bad: super::model::Step = serde_json::from_str(
        r#"{"id":"b","stage":"s","uninstall":{"kind":"terminal","cmd":"ls\nrm x"}}"#,
    )
    .unwrap();
    assert!(matches!(
        fix::uninstall_line(kun, &bad),
        Err(ReadinessError::Invalid(_))
    ));
    let empty: super::model::Step =
        serde_json::from_str(r#"{"id":"b","stage":"s","uninstall":{"kind":"terminal"}}"#).unwrap();
    assert!(fix::uninstall_line(kun, &empty).is_err());
}

fn gate_catalog() -> Catalog {
    catalog::parse(
        r#"{"version":5,"stages":["identity","engine"],"steps":[
        {"id":"gh.auth","stage":"identity","gate":true,"departments":["*"]},
        {"id":"crm.api","stage":"identity","gate":true,"gateFor":["sales","support"],"departments":["*"]},
        {"id":"pack.tech","stage":"engine","gate":true,"departments":["tech"]},
        {"id":"pack.sales","stage":"engine","gate":true,"departments":["sales"]},
        {"id":"tool.x","stage":"engine","gate":true,"os":["win"],"departments":["*"]},
        {"id":"brew","stage":"engine","departments":["*"]}]}"#,
    )
    .unwrap()
}

fn state_of(rows: &[(&str, &str)], removed: &[&str]) -> ReadinessState {
    let steps: Vec<serde_json::Value> = rows
        .iter()
        .map(|(id, st)| serde_json::json!({"id": id, "status": st}))
        .collect();
    serde_json::from_value(serde_json::json!({
        "at": "2026-11-22T06:31:15.207Z", "steps": steps, "removed": removed
    }))
    .unwrap()
}

#[test]
fn the_gate_follows_the_departments_and_gate_for() {
    let c = gate_catalog();
    let mac = |d: &[&str]| Scope { os: "mac".into(), ..scope(d, "member") };
    assert_eq!(catalog::gate_ids(&c, &mac(&["tech"])), ["gh.auth", "pack.tech"]);
    assert_eq!(catalog::gate_ids(&c, &mac(&["sales"])), ["gh.auth", "crm.api", "pack.sales"]);
    assert_eq!(
        catalog::gate_ids(&c, &mac(&["tech", "support"])),
        ["gh.auth", "crm.api", "pack.tech"]
    );
    // no profile: only the steps for everyone; the OS filter still applies
    assert_eq!(catalog::gate_ids(&c, &mac(&[])), ["gh.auth"]);
    let win = Scope { os: "win".into(), ..scope(&["tech"], "member") };
    assert_eq!(catalog::gate_ids(&c, &win), ["gh.auth", "pack.tech", "tool.x"]);
}

#[test]
fn the_gate_passes_only_when_every_gate_step_passes() {
    let c = gate_catalog();
    let sc = Scope { os: "mac".into(), ..scope(&["tech"], "member") };
    let ok = state_of(&[("gh.auth", "pass"), ("pack.tech", "fixed"), ("brew", "fail")], &[]);
    let g = catalog::gate_view(Some(&ok), &c, &sc);
    assert!(g.pass && g.failing.is_empty());
    assert_eq!(g.at, "2026-11-22T06:31:15.207Z");

    let bad = state_of(&[("gh.auth", "fail"), ("pack.tech", "human")], &[]);
    let g = catalog::gate_view(Some(&bad), &c, &sc);
    assert!(!g.pass);
    assert_eq!(g.failing, ["gh.auth", "pack.tech"]);

    // a gate step no run reported on yet is not a pass; no state fails them all
    let part = state_of(&[("gh.auth", "pass")], &[]);
    assert_eq!(catalog::gate_view(Some(&part), &c, &sc).failing, ["pack.tech"]);
    let none = catalog::gate_view(None, &c, &sc);
    assert_eq!((none.pass, none.failing.len(), none.at.as_str()), (false, 2, ""));
}

#[test]
fn a_removed_gate_step_does_not_pass_but_a_plain_skip_does() {
    let c = gate_catalog();
    let sc = Scope { os: "mac".into(), ..scope(&["tech"], "member") };
    let s = state_of(&[("gh.auth", "skip"), ("pack.tech", "skip")], &["pack.tech"]);
    assert_eq!(catalog::gate_view(Some(&s), &c, &sc).failing, ["pack.tech"]);
}

#[test]
fn a_catalog_without_gate_flags_always_passes() {
    let c = catalog::parse(r#"{"steps":[{"id":"a","stage":"s"}]}"#).unwrap();
    let g = catalog::gate_view(None, &c, &scope(&[], "member"));
    assert!(g.pass && g.steps.is_empty());
}

#[test]
fn the_view_and_the_beat_carry_the_gate() {
    let c = gate_catalog();
    let sc = Scope { os: "mac".into(), ..scope(&["tech"], "member") };
    let st = state_of(&[("gh.auth", "pass")], &[]);
    let v = catalog::view(Some(&c), &sc, Some(&st));
    assert_eq!(v.gate.unwrap().failing, ["pack.tech"]);
    assert!(catalog::view(None, &sc, None).gate.is_none());
    let b = catalog::beat(&st, Some(&c), &sc, 0);
    let json = serde_json::to_value(&b).unwrap();
    assert_eq!(json["gate"]["pass"], false);
    assert_eq!(json["gate"]["failing"][0], "pack.tech");
    assert_eq!(json["gate"]["at"], "2026-11-22T06:31:15.207Z");
    // a state without a timestamp takes the heartbeat's clock, as ISO
    let mut undated = st.clone();
    undated.at = serde_json::Value::Null;
    let b = catalog::beat(&undated, Some(&c), &sc, 1_795_000_000);
    assert_eq!(b.gate.unwrap().at, "2026-11-18T11:06:40Z");
    assert!(serde_json::to_value(catalog::beat(&st, None, &sc, 0)).unwrap().get("gate").is_none());
}

#[test]
fn iso_formats_epoch_seconds() {
    assert_eq!(catalog::iso(0), "1970-01-01T00:00:00Z");
    assert_eq!(catalog::iso(951_782_400), "2000-02-29T00:00:00Z");
}
