use std::sync::Mutex;

use super::{
    model::{Context, KunBlock, Readiness, ReadyState, Task, TaskComment, TaskStatus, TasksResponse},
    readiness::{self, Facts, readiness as verdict},
    *,
};
use crate::{
    catalog::ItemStatus,
    network::{
        host::fake::FakeHost,
        model::{LocalRepo, Receipt, RepoMode},
    },
};

fn task(id_n: u32) -> Task {
    Task {
        id: format!("databayt/hogwarts#{id_n}"),
        repo: "hogwarts".into(),
        number: id_n,
        title: "Draft the pilot proposal".into(),
        url: format!("https://github.com/databayt/hogwarts/issues/{id_n}"),
        department: "sales".into(),
        status: TaskStatus::Todo,
        holder: None,
        priority: Some(1),
        sprint: Some("Sprint 3".into()),
        order: 0,
        auto: false,
        kun: KunBlock {
            repos: vec!["hogwarts".into()],
            items: vec!["skill:proposal".into()],
            after: Vec::new(),
            driver: None,
            formula: None,
            stage: None,
        },
        after_open: Vec::new(),
        body: "Quote Al-Ibda.".into(),
        updated_at: "t".into(),
        assignee: None,
        film: None,
        formula: None,
    }
}

fn clone_of(name: &str, behind: u32) -> LocalRepo {
    LocalRepo {
        name: name.into(),
        mode: RepoMode::Keep,
        branch: "main".into(),
        behind,
        ahead: 0,
        dirty: false,
    }
}

fn ready_facts() -> Facts {
    Facts {
        me: "ali".into(),
        repos: vec![clone_of("hogwarts", 0)],
        items: vec![("skill:proposal".into(), ItemStatus::Installed)],
        allowed_items: None,
        allowed_repos: Vec::new(),
        gate_failing: Vec::new(),
    }
}

fn run(t: &Task, f: &Facts) -> Readiness {
    verdict(t, f, 0, 1)
}

#[test]
fn a_task_with_everything_in_place_is_ready() {
    let r = run(&task(1), &ready_facts());
    assert_eq!((r.state, r.missing.is_empty()), (ReadyState::Ready, true));
}

#[test]
fn missing_and_stale_repos_and_items_say_what_to_fetch() {
    let mut f = ready_facts();
    f.repos = vec![clone_of("hogwarts", 2)];
    f.items = vec![("skill:proposal".into(), ItemStatus::Outdated)];
    let r = run(&task(1), &f);
    assert_eq!(r.state, ReadyState::Missing);
    assert_eq!(r.missing, ["behind:hogwarts", "item:skill:proposal"]);
    f.repos.clear();
    f.items.clear();
    assert_eq!(
        run(&task(1), &f).missing,
        ["repo:hogwarts", "item:skill:proposal"]
    );
}

#[test]
fn open_afters_a_foreign_holder_and_a_full_wip_block() {
    let mut t = task(2);
    t.after_open = vec!["databayt/hogwarts#1".into()];
    t.holder = Some("sara".into());
    let r = verdict(&t, &ready_facts(), 1, 1);
    assert_eq!(r.state, ReadyState::Blocked);
    assert_eq!(r.missing, ["after:databayt/hogwarts#1", "held:sara", "wip"]);
}

#[test]
fn my_own_task_is_ready_even_when_wip_is_full() {
    let mut t = task(3);
    t.holder = Some("ali".into());
    t.status = TaskStatus::InProgress;
    assert_eq!(verdict(&t, &ready_facts(), 1, 1).state, ReadyState::Ready);
    // A second task is refused while one is in progress, and allowed once the CEO sets wip:2.
    assert_eq!(
        verdict(&task(4), &ready_facts(), 1, 1).state,
        ReadyState::Blocked
    );
    assert_eq!(
        verdict(&task(4), &ready_facts(), 1, 2).state,
        ReadyState::Ready
    );
}

#[test]
fn the_profile_can_forbid_items_and_repos() {
    let mut f = ready_facts();
    f.allowed_items = Some(vec!["skill:pricing".into()]);
    f.allowed_repos = vec!["mkan".into()];
    let r = run(&task(5), &f);
    assert_eq!(r.state, ReadyState::Blocked);
    assert_eq!(r.missing, ["profile:hogwarts", "profile:skill:proposal"]);
    // `databayt/<repo>` and the bare name are the same repo.
    let mut t = task(6);
    t.kun.repos = vec!["databayt/hogwarts".into()];
    assert_eq!(run(&t, &ready_facts()).state, ReadyState::Ready);
}

#[test]
fn a_machine_without_a_login_holds_nothing() {
    let mut f = ready_facts();
    f.me = String::new();
    let mut t = task(7);
    t.holder = Some("ali".into());
    assert_eq!(run(&t, &f).missing, ["held:ali"]);
}

#[test]
fn the_seed_round_trips_and_names_its_task() {
    let t = task(8);
    let text = readiness::seed_text(&t);
    assert!(
        text.contains("# Draft the pilot proposal")
            && text.contains("Quote Al-Ibda.")
            && text.contains("KNOWLEDGE.md")
    );
    assert_eq!(
        readiness::seed_repos(&text, &t.id),
        Some(vec!["hogwarts".to_owned()])
    );
    assert_eq!(
        readiness::seed_repos(&text, "databayt/hogwarts#9"),
        None,
        "another task's seed is not this one's"
    );
    assert_eq!(readiness::seed_repos("# not a seed", &t.id), None);
}

/// Serves a canned task list and records the claims.
struct Server {
    tasks: TasksResponse,
    claim: Result<String, (u16, String)>,
    detail: Result<String, (u16, String)>,
    posts: Mutex<Vec<(String, String)>>,
}

impl Api for Server {
    async fn get(&self, path: &str) -> Result<String, ApiError> {
        if path.starts_with("/api/machine/task?id=") {
            return self.detail.clone().map_err(|(status, body)| ApiError::Status { status, body });
        }
        assert!(path.starts_with("/api/machine/tasks?department="), "{path}");
        Ok(serde_json::to_string(&self.tasks).unwrap())
    }

    async fn post(&self, path: &str, body: String) -> Result<String, ApiError> {
        self.posts.lock().unwrap().push((path.to_owned(), body));
        self.claim
            .clone()
            .map_err(|(status, body)| ApiError::Status { status, body })
    }
}

fn server(board: Vec<Task>, in_progress: u32) -> Server {
    Server {
        tasks: TasksResponse {
            sprint: None,
            mine: Vec::new(),
            board,
            in_progress,
            wip_limit: 1,
        },
        claim: Ok(String::new()),
        detail: Err((404, "no detail".into())),
        posts: Mutex::new(Vec::new()),
    }
}

fn depts() -> Vec<String> {
    vec!["sales".into()]
}

#[tokio::test]
async fn start_claims_then_writes_the_seed() {
    let t = task(10);
    let mut s = server(vec![t.clone()], 0);
    s.claim = Ok(serde_json::json!({ "ok": true, "task": { "id": t.id, "repo": "hogwarts", "number": 10, "title": "Draft the pilot proposal", "url": "u",
        "department": "sales", "status": "in-progress", "holder": "ali", "priority": 1, "sprint": null, "order": 0,
        "kun": { "repos": ["hogwarts"], "items": [], "after": [], "driver": null }, "afterOpen": [], "body": "b", "updatedAt": "t" } }).to_string());
    let dir = tempfile::tempdir().unwrap();
    let started = start_with(&s, &ready_facts(), &depts(), dir.path(), &t.id)
        .await
        .unwrap();
    assert_eq!(started.status, TaskStatus::InProgress);
    assert_eq!(s.posts.lock().unwrap()[0].0, "/api/machine/claim");
    let seed = fs::read_to_string(dir.path().join(SEED_FILE)).unwrap();
    assert_eq!(
        readiness::seed_repos(&seed, &t.id),
        Some(vec!["hogwarts".to_owned()])
    );
}

#[tokio::test]
async fn start_refuses_before_claiming_when_the_gate_says_no() {
    let t = task(11);
    let s = server(vec![t.clone()], 1);
    let dir = tempfile::tempdir().unwrap();
    let err = start_with(&s, &ready_facts(), &depts(), dir.path(), &t.id)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, TaskError::NotReady(m) if m == &["wip"]),
        "{err}"
    );
    assert!(s.posts.lock().unwrap().is_empty(), "nothing was claimed");
    assert!(!dir.path().join(SEED_FILE).exists());
    let gone = start_with(
        &s,
        &ready_facts(),
        &depts(),
        dir.path(),
        "databayt/hogwarts#99",
    )
    .await
    .unwrap_err();
    assert!(matches!(gone, TaskError::NotFound));
    let bad = start_with(&s, &ready_facts(), &depts(), dir.path(), "x")
        .await
        .unwrap_err();
    assert!(matches!(bad, TaskError::Invalid(_)));
}

#[tokio::test]
async fn a_server_refusal_is_surfaced_and_leaves_no_seed() {
    let t = task(12);
    let mut s = server(vec![t.clone()], 0);
    s.claim = Err((409, r#"{"error":"held","detail":"sara has it"}"#.into()));
    let dir = tempfile::tempdir().unwrap();
    let err = start_with(&s, &ready_facts(), &depts(), dir.path(), &t.id)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, TaskError::Refused(m) if m == "held: sara has it"),
        "{err}"
    );
    assert!(!dir.path().join(SEED_FILE).exists());
}

#[tokio::test]
async fn restarting_my_own_task_skips_the_claim() {
    let mut t = task(13);
    t.holder = Some("ali".into());
    t.status = TaskStatus::InProgress;
    let s = server(vec![t.clone()], 1);
    let dir = tempfile::tempdir().unwrap();
    start_with(&s, &ready_facts(), &depts(), dir.path(), &t.id)
        .await
        .unwrap();
    assert!(s.posts.lock().unwrap().is_empty());
    assert!(dir.path().join(SEED_FILE).exists());
}

#[tokio::test]
async fn release_and_done_validate_and_post() {
    let mut s = server(Vec::new(), 0);
    s.claim = Ok(r#"{"ok":true}"#.into());
    release_with(&s, "databayt/kun#3").await.unwrap();
    finish_with(&s, "databayt/kun#3", "  shipped  ")
        .await
        .unwrap();
    {
        let posts = s.posts.lock().unwrap();
        assert_eq!(posts[0].0, "/api/machine/release");
        assert_eq!(posts[1].0, "/api/machine/done");
        let body: serde_json::Value = serde_json::from_str(&posts[1].1).unwrap();
        assert_eq!(
            body,
            serde_json::json!({ "id": "databayt/kun#3", "summary": "shipped" })
        );
    }
    assert!(matches!(
        finish_with(&s, "databayt/kun#3", "  ").await,
        Err(TaskError::Invalid(_))
    ));
    assert!(matches!(
        release_with(&s, "kun#3").await,
        Err(TaskError::Invalid(_))
    ));
    let mut no = server(Vec::new(), 0);
    no.claim = Ok(r#"{"ok":false,"error":"not-found"}"#.into());
    assert!(
        matches!(release_with(&no, "databayt/kun#3").await, Err(TaskError::Refused(m)) if m == "not-found")
    );
}

#[test]
fn facts_come_from_the_host() {
    let host = FakeHost {
        login: "ali".into(),
        repos: vec![clone_of("hogwarts", 0)],
        items: vec![("skill:proposal".into(), ItemStatus::Installed)],
        allowed_repos: vec!["hogwarts".into()],
        ..FakeHost::default()
    };
    let f = gather_facts(&host);
    assert_eq!(
        (
            f.me.as_str(),
            f.repos.len(),
            f.items.len(),
            f.allowed_repos.len()
        ),
        ("ali", 1, 1, 1)
    );
}

#[test]
fn a_tasks_repo_dirs_are_the_matching_local_clones() {
    let host = FakeHost {
        paths: vec!["/w/repos/hogwarts".into(), "/w/repos/mkan".into()],
        ..FakeHost::default()
    };
    let dirs = task_repo_dirs(&host, &["hogwarts".into(), "kun".into()]);
    assert_eq!(dirs, vec![PathBuf::from("/w/repos/hogwarts")]);
}

fn comment(body: &str, receipt: bool) -> TaskComment {
    TaskComment {
        author: "ali".into(),
        body: body.into(),
        created_at: "2026-11-19".into(),
        receipt,
    }
}

#[test]
fn the_kun_block_reads_a_known_driver_and_ignores_an_unknown_one() {
    let k: KunBlock = serde_json::from_str(r#"{"repos":[],"driver":"Codex"}"#).unwrap();
    assert_eq!(k.driver.as_deref(), Some("codex"));
    let k: KunBlock = serde_json::from_str(r#"{"driver":"gpt9"}"#).unwrap();
    assert_eq!(k.driver, None);
    let k: KunBlock = serde_json::from_str(r#"{"driver":null}"#).unwrap();
    assert_eq!(k.driver, None);
    let k: KunBlock = serde_json::from_str("{}").unwrap();
    assert_eq!(k.driver, None);
    // An older server without `auto` still decodes.
    let t = serde_json::to_value(task(1)).unwrap();
    let mut t = t.as_object().unwrap().clone();
    t.remove("auto");
    assert!(!serde_json::from_value::<Task>(t.into()).unwrap().auto);
}

#[test]
fn a_blocked_task_is_blocked_by_its_open_afters_even_when_first_in_the_queue() {
    let mut t = task(20);
    t.order = 0;
    t.after_open = vec!["databayt/hogwarts#19".into()];
    let r = run(&t, &ready_facts());
    assert_eq!(
        (r.state, r.missing),
        (ReadyState::Blocked, vec!["after:databayt/hogwarts#19".to_owned()])
    );
}

fn auto(n: u32, order: u32) -> Task {
    let mut t = task(n);
    t.auto = true;
    t.order = order;
    t
}

#[test]
fn next_auto_task_takes_the_first_free_unblocked_auto_task_in_queue_order() {
    let mut blocked = auto(30, 0);
    blocked.after_open = vec!["databayt/hogwarts#1".into()];
    let mut held = auto(31, 1);
    held.holder = Some("sara".into());
    let manual = task(32); // not run:auto
    let late = auto(33, 4);
    let first = auto(34, 3);
    let board = vec![blocked, held, manual, late.clone(), first.clone()];
    assert_eq!(readiness::next_auto_task(&board, &[]), Some(&first));
    // Sorted by `order`, not by position.
    let shuffled = vec![late, first.clone()];
    assert_eq!(readiness::next_auto_task(&shuffled, &[]), Some(&first));
    assert_eq!(readiness::next_auto_task(&[task(35)], &[]), None);
}

#[test]
fn next_auto_task_waits_while_one_is_in_progress() {
    let mut mine = auto(40, 0);
    mine.status = TaskStatus::InProgress;
    mine.holder = Some("ali".into());
    let board = vec![auto(41, 1)];
    assert_eq!(readiness::next_auto_task(&board, &[mine]), None);
    let mut in_review = auto(42, 0);
    in_review.status = TaskStatus::Review;
    assert!(readiness::next_auto_task(&board, &[in_review]).is_some());
}

#[test]
fn the_aware_excerpt_keeps_only_lines_about_the_repos_and_at_most_forty() {
    let text = "Hogwarts deploy green\nmkan queue 3\nunrelated\n";
    assert_eq!(
        readiness::aware_excerpt(text, &["databayt/hogwarts".into()]).as_deref(),
        Some("Hogwarts deploy green")
    );
    assert_eq!(readiness::aware_excerpt(text, &["kun".into()]), None);
    assert_eq!(readiness::aware_excerpt(text, &[]), None);
    let many = "kun line\n".repeat(60);
    assert_eq!(
        readiness::aware_excerpt(&many, &["kun".into()]).unwrap().lines().count(),
        40
    );
}

#[test]
fn the_seed_carries_comments_with_receipts_marked_and_the_state_excerpt() {
    let mut t = task(50);
    t.kun.driver = Some("codex".into());
    let ctx = Context {
        comments: vec![comment("looks fine", false), comment("did it", true)],
        aware: Some("hogwarts deploy green".into()),
    };
    let text = readiness::seed_text_with(&t, &ctx);
    assert!(text.contains("- Driver: codex"));
    assert!(text.contains("## Recent comments") && text.contains("looks fine"));
    assert!(text.contains("ali · 2026-11-19 [run receipt]"));
    assert!(text.contains("## Machine state") && text.contains("hogwarts deploy green"));
    assert_eq!(readiness::seed_repos(&text, &t.id), Some(vec!["hogwarts".to_owned()]));
    // No context: no extra sections.
    let plain = readiness::seed_text(&t);
    assert!(!plain.contains("Recent comments") && !plain.contains("Machine state"));
}

#[tokio::test]
async fn start_writes_the_context_pack_when_kun_serves_it() {
    let mut t = task(51);
    t.holder = Some("ali".into());
    t.status = TaskStatus::InProgress;
    let mut s = server(vec![t.clone()], 1);
    s.detail = Ok(serde_json::json!({ "task": t, "comments": [
        { "author": "ceo", "body": "mind the VAT", "createdAt": "d", "receipt": false },
        { "author": "ali", "body": "<!-- kun:receipt -->\nran it", "createdAt": "d2", "receipt": true } ] })
    .to_string());
    let dir = tempfile::tempdir().unwrap();
    let aware = dir.path().join("aware.md");
    fs::write(&aware, "hogwarts: 2 open\nother: x\n").unwrap();
    start_with_aware(&s, &ready_facts(), &depts(), dir.path(), &t.id, Some(&aware))
        .await
        .unwrap();
    let seed = fs::read_to_string(dir.path().join(SEED_FILE)).unwrap();
    assert!(seed.contains("mind the VAT") && seed.contains("[run receipt]"));
    assert!(seed.contains("hogwarts: 2 open") && !seed.contains("other: x"));
    assert!(seed.contains("Quote Al-Ibda."), "the issue body is still there");
}

fn receipt(id: &str) -> Receipt {
    Receipt {
        id: id.into(),
        driver: "claude".into(),
        model: "opus".into(),
        duration_ms: 90_000,
        commits: vec!["abc123".into()],
        artifacts: Vec::new(),
        summary: "did it".into(),
        next: None,
        formula: None,
        stage: None,
        hashes: None,
    }
}

#[tokio::test]
async fn a_receipt_is_validated_posted_and_a_refusal_surfaced() {
    let mut s = server(Vec::new(), 0);
    s.claim = Ok(r#"{"ok":true}"#.into());
    receipt_with(&s, &receipt("databayt/kun#3")).await.unwrap();
    {
        let posts = s.posts.lock().unwrap();
        assert_eq!(posts[0].0, "/api/machine/receipt");
        let body: serde_json::Value = serde_json::from_str(&posts[0].1).unwrap();
        assert_eq!(body["durationMs"], 90_000);
        assert!(body.get("next").is_none(), "an absent next is not sent");
    }
    let mut bad = receipt("databayt/kun#3");
    bad.summary = "x".repeat(4001);
    assert!(matches!(receipt_with(&s, &bad).await, Err(TaskError::Invalid(_))));
    bad = receipt("kun#3");
    assert!(matches!(receipt_with(&s, &bad).await, Err(TaskError::Invalid(_))));
    bad = receipt("databayt/kun#3");
    bad.driver = "gpt9".into();
    assert!(matches!(receipt_with(&s, &bad).await, Err(TaskError::Invalid(_))));
    assert_eq!(s.posts.lock().unwrap().len(), 1, "invalid receipts never reach the wire");

    let mut no = server(Vec::new(), 0);
    no.claim = Err((409, r#"{"ok":false,"error":"held","detail":"you do not hold it"}"#.into()));
    assert!(
        matches!(receipt_with(&no, &receipt("databayt/kun#3")).await, Err(TaskError::Refused(m)) if m == "held: you do not hold it")
    );
}

#[test]
fn a_task_tolerates_the_new_optional_fields_being_absent() {
    let base = serde_json::to_value(task(9)).unwrap();
    let mut old = base.clone();
    for k in ["assignee", "film", "formula"] {
        old.as_object_mut().unwrap().remove(k);
    }
    let t: Task = serde_json::from_value(old).unwrap();
    assert_eq!((t.assignee, t.film, t.formula), (None, None, None));
    let mut new = base;
    new["assignee"] = "sedon".into();
    new["film"] = "databayt/hogwarts#12".into();
    new["formula"] = "video.order.blender".into();
    let t: Task = serde_json::from_value(new).unwrap();
    assert_eq!(t.assignee.as_deref(), Some("sedon"));
    assert_eq!(t.film.as_deref(), Some("databayt/hogwarts#12"));
    assert_eq!(t.formula.as_deref(), Some("video.order.blender"));
    // The kun block's formula= and stage= parse too, and are absent on older servers.
    let k: KunBlock = serde_json::from_value(serde_json::json!({"repos":["hogwarts"],"formula":"video.order.blender","stage":"visual"})).unwrap();
    assert_eq!((k.formula.as_deref(), k.stage.as_deref()), (Some("video.order.blender"), Some("visual")));
    assert_eq!(serde_json::from_str::<KunBlock>("{}").unwrap().formula, None);
}

#[tokio::test]
async fn an_order_assigned_to_me_outside_my_department_is_in_the_queue_and_startable() {
    let mut order = task(40);
    order.department = "video".into();
    order.assignee = Some("ali".into());
    order.film = Some("databayt/hogwarts#12".into());
    let mut s = server(Vec::new(), 0);
    s.tasks.mine = vec![order];
    s.claim = Ok(r#"{"ok":true}"#.into());
    // The server returns it for a department the caller is not in; the gate does not care.
    let view = view_with(&s, &ready_facts(), "tech").await.unwrap();
    assert_eq!(view.tasks.mine[0].assignee.as_deref(), Some("ali"));
    assert_eq!(view.readiness["databayt/hogwarts#40"].state, ReadyState::Ready);
    let dir = tempfile::tempdir().unwrap();
    let started = start_with(&s, &ready_facts(), &["tech".to_owned()], dir.path(), "databayt/hogwarts#40").await.unwrap();
    assert_eq!(started.department, "video");
}

#[test]
fn any_4xx_with_a_reason_is_a_refusal_and_a_bare_one_is_not() {
    let e = |status, body: &str| refusal(ApiError::Status { status, body: body.into() });
    assert!(matches!(e(413, r#"{"error":"too-large","detail":"over 5 GB"}"#), TaskError::Refused(m) if m == "too-large: over 5 GB"));
    assert!(matches!(e(409, "plain text"), TaskError::Refused(m) if m == "plain text"));
    assert!(matches!(e(404, "plain text"), TaskError::Api(_)));
    assert!(matches!(e(500, r#"{"error":"boom"}"#), TaskError::Api(_)));
}

#[test]
fn a_failing_gate_blocks_every_task_with_doctor_reasons() {
    let mut f = ready_facts();
    f.gate_failing = vec!["launcher.c".into(), "pack.tech".into()];
    let r = run(&task(1), &f);
    assert_eq!(r.state, ReadyState::Blocked);
    assert_eq!(r.missing, ["doctor:launcher.c", "doctor:pack.tech"]);
    // missing repos or items stay listed after the doctor reasons, still blocked
    f.repos.clear();
    let r = run(&task(1), &f);
    assert_eq!(r.state, ReadyState::Blocked);
    assert_eq!(r.missing[..2], ["doctor:launcher.c", "doctor:pack.tech"]);
    assert!(r.missing.iter().any(|m| m.starts_with("repo:")));
    // a passing gate leaves the verdict alone
    assert_eq!(run(&task(1), &ready_facts()).state, ReadyState::Ready);
}

#[tokio::test]
async fn the_formula_and_stage_come_from_the_task() {
    let mut t = task(41);
    t.kun.formula = Some("video.order.capture".into());
    t.kun.stage = Some("assets".into());
    let mut s = server(Vec::new(), 0);
    s.detail = Ok(serde_json::to_string(&serde_json::json!({ "task": t, "comments": [] })).unwrap());
    assert_eq!(task_labels(&s, "databayt/hogwarts#41").await, (Some("video.order.capture".into()), Some("assets".into())));
    // Task.formula wins over the block's; a failed lookup gives none.
    t.formula = Some("video.order.blender".into());
    s.detail = Ok(serde_json::to_string(&serde_json::json!({ "task": t, "comments": [] })).unwrap());
    assert_eq!(task_labels(&s, "databayt/hogwarts#41").await.0.as_deref(), Some("video.order.blender"));
    s.detail = Err((404, "no".into()));
    assert_eq!(task_labels(&s, "databayt/hogwarts#41").await, (None, None));
}
