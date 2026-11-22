use super::*;

#[test]
fn titles_are_one_short_line() {
    assert_eq!(title_from("  اكتب \n عرض   سعر "), "اكتب عرض سعر");
    let long = "x".repeat(100);
    assert_eq!(title_from(&long).chars().count(), 61);
    assert_eq!(title_from("   "), "…");
}

#[test]
fn job_ids_are_validated_and_the_job_is_built() {
    for bad in ["", "a b", "../x", &"a".repeat(65)] {
        assert!(
            matches!(
                build_job(
                    bad,
                    DriverKind::Claude,
                    "hi".into(),
                    ".".into(),
                    None,
                    Vec::new(),
                    Vec::new()
                ),
                Err(ChatError::BadJobId)
            ),
            "{bad}"
        );
    }
    let job = build_job(
        "j-1",
        DriverKind::Codex,
        "hi".into(),
        "/tmp".into(),
        Some("s1".into()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        (job.driver, job.cwd.as_str(), job.session.as_deref()),
        (DriverKind::Codex, "/tmp", Some("s1"))
    );
}

#[test]
fn cancel_reaches_a_registered_job_once() {
    let (tx, mut rx) = oneshot::channel();
    let reg = register("test-cancel", tx).unwrap();
    let (tx2, _rx2) = oneshot::channel();
    assert!(matches!(
        register("test-cancel", tx2),
        Err(ChatError::Duplicate)
    ));
    assert!(cancel("test-cancel"));
    assert!(rx.try_recv().is_ok());
    assert!(!cancel("test-cancel"));
    drop(reg);
    assert!(!cancel("never-ran"));
}

#[test]
fn registration_is_released_on_drop() {
    let (tx, _rx) = oneshot::channel();
    drop(register("test-drop", tx).unwrap());
    let (tx, _rx) = oneshot::channel();
    assert!(register("test-drop", tx).is_ok());
}

fn resolved(driver: DriverKind, locked: bool) -> Resolved {
    use crate::profile::{Access, MachineProfile};
    Resolved {
        profile: MachineProfile {
            login: "moed".into(),
            name: String::new(),
            os: "windows".into(),
            driver,
            departments: Vec::new(),
            department: String::new(),
            repos: vec!["kun".into(), "mkan".into()],
            access: Access::FULL,
            hidden: Vec::new(),
        },
        secrets: Vec::new(),
        driver_locked: locked,
        explicit: true,
    }
}

#[test]
fn a_locked_profile_fixes_the_driver() {
    let locked = resolved(DriverKind::Claude, true);
    assert_eq!(
        pick_driver(&locked, Some(DriverKind::Codex), DriverKind::Opencode),
        DriverKind::Claude
    );
    let open = resolved(DriverKind::Claude, false);
    assert_eq!(
        pick_driver(&open, Some(DriverKind::Codex), DriverKind::Opencode),
        DriverKind::Codex
    );
    assert_eq!(
        pick_driver(&open, None, DriverKind::Opencode),
        DriverKind::Opencode
    );
}

#[test]
fn the_job_sees_only_the_cloned_repos_and_the_settings_file() {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace {
        dir: root.path().join("moed"),
        repos: root.path().join("repos"),
        work: root.path().join("moed/work"),
        settings_file: root.path().join("moed/.claude-settings.json"),
    };
    std::fs::create_dir_all(ws.repos.join("kun")).unwrap();
    let repos = ["kun".to_owned(), "mkan".to_owned()];
    let extras = job_extras(DriverKind::Claude, &repos, &ws);
    assert_eq!(
        extras,
        vec![
            "--add-dir".to_owned(),
            ws.repos.join("kun").display().to_string(),
            "--settings".into(),
            ws.settings_file.display().to_string(),
        ]
    );
    assert!(job_extras(DriverKind::Codex, &repos, &ws).is_empty());
    let job = build_job(
        "j1",
        DriverKind::Claude,
        "hi".into(),
        ".".into(),
        None,
        extras,
        vec![("GEMINI_API_KEY".into(), "k".into())],
    )
    .unwrap();
    assert!(driver::args_for(&job).is_ok());
}

#[test]
fn a_task_run_adds_its_repos_once_and_only_for_claude() {
    let dirs = [
        PathBuf::from("/w/repos/hogwarts"),
        PathBuf::from("/w/repos/kun"),
    ];
    let have = vec![
        "--add-dir".to_owned(),
        "/w/repos/kun".to_owned(),
        "--settings".to_owned(),
        "/w/s.json".to_owned(),
    ];
    assert_eq!(
        task_extras(DriverKind::Claude, &have, &dirs),
        ["--add-dir", "/w/repos/hogwarts"]
    );
    assert!(task_extras(DriverKind::Codex, &have, &dirs).is_empty());
    // What it builds passes the driver's own whitelist.
    let mut all = have.clone();
    all.extend(task_extras(DriverKind::Claude, &have, &dirs));
    assert!(
        build_job(
            "j",
            DriverKind::Claude,
            "hi".into(),
            ".".into(),
            None,
            all,
            Vec::new()
        )
        .is_ok()
    );
    assert!(
        driver::args_for(
            &build_job(
                "j",
                DriverKind::Claude,
                "hi".into(),
                ".".into(),
                None,
                vec!["--add-dir".into(), "/w/repos/hogwarts".into()],
                Vec::new()
            )
            .unwrap()
        )
        .is_ok()
    );
}

#[test]
fn a_task_run_needs_the_seed_that_start_wrote() {
    use crate::network::host::fake::FakeHost;
    let host = FakeHost {
        paths: vec!["/w/repos/hogwarts".into()],
        ..FakeHost::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().display().to_string();
    let id = "databayt/hogwarts#10";
    assert!(matches!(
        task_setup(&cwd, "nope", DriverKind::Claude, &[], &host),
        Err(ChatError::BadTask)
    ));
    assert!(matches!(
        task_setup(&cwd, id, DriverKind::Claude, &[], &host),
        Err(ChatError::TaskNotStarted)
    ));
    std::fs::write(
        dir.path().join(SEED_FILE),
        format!("<!-- kun-task: {id} repos=hogwarts,kun -->\n# t\n"),
    )
    .unwrap();
    assert_eq!(
        task_setup(&cwd, id, DriverKind::Claude, &[], &host).unwrap(),
        ["--add-dir", "/w/repos/hogwarts"]
    );
    assert!(matches!(
        task_setup(&cwd, "databayt/hogwarts#11", DriverKind::Claude, &[], &host),
        Err(ChatError::TaskNotStarted)
    ));
}

#[test]
fn the_task_prompt_points_at_the_seed() {
    let p = task_prompt("databayt/kun#3", "start");
    assert!(p.contains("TASK.md") && p.contains("databayt/kun#3") && p.ends_with("start"));
}

fn ws_in(root: &Path) -> Workspace {
    Workspace {
        dir: root.join("moed"),
        repos: root.join("repos"),
        work: root.join("moed/work"),
        settings_file: root.join("moed/.claude-settings.json"),
    }
}

/// A kun checkout with a registry (kun, mkan, souq, ios-app active; secret planned) and a pack for `video`.
fn kun_fixture() -> tempfile::TempDir {
    let kun = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(kun.path().join(".claude/memory")).unwrap();
    std::fs::write(
        kun.path().join(".claude/memory/repositories.json"),
        r#"{"repositories":{"core":[{"id":"kun","status":"active"},{"id":"machine","status":"active"}],
            "products":[{"id":"hogwarts","status":"active"},{"id":"mkan","status":"active"},{"id":"souq","status":"active"},
                        {"id":"secret","status":"planned"}],
            "specialized":[{"id":"ios-app","status":"active"},{"id":"marketing","status":"active","word":"landing"}]}}"#,
    )
    .unwrap();
    let dept = kun.path().join("departments/video");
    std::fs::create_dir_all(&dept).unwrap();
    std::fs::write(dept.join("pack.json"), r#"{"repos":["kun","hogwarts","marketing"]}"#).unwrap();
    kun
}

#[test]
fn a_repo_limits_the_run_to_that_repo_and_must_be_known_and_cloned() {
    let kun = kun_fixture();
    let root = tempfile::tempdir().unwrap();
    let ws = ws_in(root.path());
    for n in ["kun", "mkan", "secret", "souq"] {
        std::fs::create_dir_all(ws.repos.join(n)).unwrap();
    }
    let r = resolved(DriverKind::Claude, true);
    let check = |dept: &str, repo: Option<&str>| validate_repo(&r, dept, repo, &ws.repos, kun.path());
    // On the menu and cloned.
    assert_eq!(check("tech", Some("kun")).unwrap().as_deref(), Some("kun"));
    // None or empty means all repos.
    assert_eq!(check("tech", None).unwrap(), None);
    assert_eq!(check("tech", Some("")).unwrap(), None);
    // Cloned but not on the menu (planned in the registry, or not the profile's); or on it but not cloned; or unsafe.
    for bad in ["secret", "souq", "hogwarts", "../kun"] {
        assert!(matches!(check("tech", Some(bad)), Err(ChatError::BadRepo)), "{bad}");
    }
    std::fs::remove_dir_all(ws.repos.join("mkan")).unwrap();
    assert!(matches!(check("tech", Some("mkan")), Err(ChatError::BadRepo)));
    // The department's pack repos join the profile's.
    std::fs::create_dir_all(ws.repos.join("hogwarts")).unwrap();
    assert!(check("video", Some("hogwarts")).is_ok());
    assert!(matches!(check("tech", Some("hogwarts")), Err(ChatError::BadRepo)));
    // The extras: only that repo's --add-dir (not mkan's, though the profile lists it).
    let all = job_extras(DriverKind::Claude, &r.profile.repos, &ws);
    let one = job_extras(DriverKind::Claude, &["kun".to_owned()], &ws);
    assert_eq!(
        one,
        vec![
            "--add-dir".to_owned(),
            ws.repos.join("kun").display().to_string(),
            "--settings".into(),
            ws.settings_file.display().to_string(),
        ]
    );
    assert_eq!(all, one, "mkan is no longer cloned, so both agree here");
    std::fs::create_dir_all(ws.repos.join("mkan")).unwrap();
    let all = job_extras(DriverKind::Claude, &r.profile.repos, &ws);
    assert_eq!(all.iter().filter(|a| *a == "--add-dir").count(), 2);
}

#[test]
fn the_default_profile_takes_its_repos_from_the_department() {
    use crate::profile::{Department, RepoTag};
    let kun = kun_fixture();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("souq")).unwrap();
    let mut r = resolved(DriverKind::Claude, false);
    r.profile.repos = Vec::new();
    r.profile.departments = vec![Department {
        name: "tech".into(),
        title: "Tech".into(),
        title_ar: None,
        quick_actions: Vec::new(),
        skills: Vec::new(),
        cards: Vec::new(),
        repos: vec![RepoTag { name: "souq".into(), blocks: Vec::new() }],
    }];
    assert!(validate_repo(&r, "tech", Some("souq"), root.path(), kun.path()).is_ok());
    assert!(validate_repo(&r, "sales", Some("souq"), root.path(), kun.path()).is_err());
}

#[test]
fn the_menu_is_the_pack_and_profile_repos_that_are_active() {
    let kun = kun_fixture();
    let mut r = resolved(DriverKind::Claude, false);
    r.profile.repos = vec!["mkan".into(), "kun".into(), "secret".into(), "ghost".into(), "../x".into()];
    let names = |dept: &str| {
        repo_menu::menu_for(kun.path(), &r.profile, dept)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect::<Vec<_>>()
    };
    // video: its pack (kun, hogwarts, marketing) then the profile's; hogwarts first; planned, unknown and unsafe are gone.
    assert_eq!(names("video"), ["hogwarts", "kun", "marketing", "mkan"]);
    // A department with no pack gets the profile's repos alone.
    assert_eq!(names("tech"), ["mkan", "kun"]);
    // The registry's word shows; the name stays.
    let landing = repo_menu::menu_for(kun.path(), &r.profile, "video").unwrap();
    let m = landing.iter().find(|e| e.name == "marketing").unwrap();
    assert_eq!(m.word.as_deref(), Some("landing"));
    assert!(landing.iter().find(|e| e.name == "kun").unwrap().word.is_none());
}

#[test]
fn with_no_registry_the_menu_is_not_filtered() {
    let mut r = resolved(DriverKind::Claude, false);
    r.profile.repos = vec!["kun".into(), "mkan".into(), "hogwarts".into()];
    let none = tempfile::tempdir().unwrap();
    let menu = repo_menu::menu_for(none.path(), &r.profile, "tech").unwrap();
    assert_eq!(menu.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["hogwarts", "kun", "mkan"]);
    // A damaged registry reads as none.
    assert!(repo_menu::parse_registry("not json").is_none());
    assert!(repo_menu::parse_registry(r#"{"repositories":{"a":[]}}"#).is_none());
    // A flat array is accepted too.
    let flat = repo_menu::parse_registry(r#"{"repositories":[{"id":"kun","status":"active"}]}"#).unwrap();
    assert_eq!(repo_menu::menu(Some(&flat), &["kun".into(), "mkan".into()], &[]).len(), 1);
}

#[test]
fn the_real_registry_gives_a_menu_with_hogwarts_first() {
    let Ok(kun) = pack::kun_dir() else { return };
    let Some(reg) = repo_menu::read_registry(&kun) else {
        eprintln!("skipped: no kun clone at {}", kun.display());
        return;
    };
    let all: Vec<String> = ["marketing", "kun", "hogwarts", "sijillee"].map(str::to_owned).to_vec();
    let menu = repo_menu::menu(Some(&reg), &all, &[]);
    let names: Vec<_> = menu.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names.first(), Some(&"hogwarts"));
    assert!(!names.contains(&"sijillee"), "planned in the registry");
    assert_eq!(menu.iter().find(|e| e.name == "marketing").and_then(|e| e.word.as_deref()), Some("landing"));
}

#[test]
fn only_a_new_session_gets_the_working_in_line() {
    assert_eq!(
        with_preface("hi".into(), Some("kun"), "tech", true),
        "Working in kun for the tech department.\n\nhi"
    );
    assert_eq!(with_preface("hi".into(), Some("kun"), "tech", false), "hi");
    assert_eq!(with_preface("hi".into(), None, "tech", true), "hi");
}

#[test]
fn pack_dirs_become_deduped_add_dir_pairs_within_the_cap() {
    let dirs = [PathBuf::from("/h/media"), PathBuf::from("/h/repos/kun")];
    let have = vec!["--add-dir".to_owned(), "/h/repos/kun".to_owned()];
    assert_eq!(
        task_extras(DriverKind::Claude, &have, &dirs),
        ["--add-dir", "/h/media"]
    );
    let full: Vec<String> = (0..MAX_EXTRA_PAIRS)
        .flat_map(|i| ["--add-dir".to_owned(), format!("/h/d{i}")])
        .collect();
    assert!(task_extras(DriverKind::Claude, &full, &dirs).is_empty());
}

fn route_meta(json: &str) -> pack::PackMeta {
    let kun = tempfile::tempdir().unwrap();
    let dept = kun.path().join("departments/video");
    std::fs::create_dir_all(&dept).unwrap();
    std::fs::write(dept.join("pack.json"), json).unwrap();
    pack::meta_in(kun.path(), "video").unwrap()
}

const ROUTED: &str = r#"{"routes":[
    {"id":"plan","mode":"plan","model":"opus","effort":"high","agent":"producer","suffix":"Plan only."},
    {"id":"do","mode":"auto","suffix":""}],"defaultRoute":"plan"}"#;

#[test]
fn a_route_sets_the_mode_flags_and_suffix() {
    let meta = route_meta(ROUTED);
    let (mode, flags, suffix) = resolve_turn(&meta, DriverKind::Claude, Some("plan"), None).unwrap();
    assert_eq!((mode, suffix.as_str()), (PermissionMode::Plan, "Plan only."));
    let prompt = with_suffix("make a film".into(), &suffix);
    assert_eq!(prompt, "make a film\n\nPlan only.");
    let mut job = build_job("j-1", DriverKind::Claude, prompt, "/".into(), None, flags, Vec::new()).unwrap();
    job.permission_mode = mode;
    let args = driver::args_for(&job).unwrap();
    assert!(args.windows(2).any(|w| w == ["--permission-mode", "plan"]));
    assert!(args.windows(6).any(|w| w == ["--agent", "producer", "--model", "opus", "--effort", "high"])
        || (args.windows(2).any(|w| w == ["--agent", "producer"])
            && args.windows(2).any(|w| w == ["--model", "opus"])
            && args.windows(2).any(|w| w == ["--effort", "high"])));
    let (mode, flags, suffix) = resolve_turn(&meta, DriverKind::Claude, Some("do"), None).unwrap();
    assert_eq!((mode, flags.is_empty(), suffix.is_empty()), (PermissionMode::Auto, true, true));
    assert_eq!(with_suffix("x".into(), ""), "x");
}

#[test]
fn an_unknown_route_is_refused() {
    let meta = route_meta(ROUTED);
    assert!(matches!(
        resolve_turn(&meta, DriverKind::Claude, Some("ghost"), None),
        Err(ChatError::BadRoute(id)) if id == "ghost"
    ));
    assert!(matches!(
        resolve_turn(&meta, DriverKind::Codex, Some("ghost"), None),
        Err(ChatError::BadRoute(_))
    ));
}

#[test]
fn the_old_mode_picks_plan_or_do_and_other_drivers_ignore_routes() {
    let meta = route_meta(ROUTED);
    let (m, f, s) = resolve_turn(&meta, DriverKind::Claude, None, Some("plan")).unwrap();
    assert_eq!((m, f.is_empty(), s.as_str()), (PermissionMode::Plan, false, "Plan only."));
    let (m, f, _) = resolve_turn(&meta, DriverKind::Claude, None, Some("auto")).unwrap();
    assert_eq!((m, f.is_empty()), (PermissionMode::Auto, true));
    // No route, no mode: a plain auto run.
    let (m, f, s) = resolve_turn(&meta, DriverKind::Claude, None, None).unwrap();
    assert_eq!((m, f.is_empty(), s.is_empty()), (PermissionMode::Auto, true, true));
    // Another driver takes the route's mode but none of its flags or suffix.
    let (m, f, s) = resolve_turn(&meta, DriverKind::Codex, Some("plan"), None).unwrap();
    assert_eq!((m, f.is_empty(), s.is_empty()), (PermissionMode::Plan, true, true));
    // A bare `run` block still answers the old mode through the derived routes.
    let meta = route_meta(r#"{"run":{"plan":{"effort":"high"},"work":{"effort":"low"}}}"#);
    let (_, f, _) = resolve_turn(&meta, DriverKind::Claude, None, Some("plan")).unwrap();
    assert_eq!(f, ["--effort", "high"]);
    let (_, f, _) = resolve_turn(&meta, DriverKind::Claude, None, Some("auto")).unwrap();
    assert_eq!(f, ["--effort", "low"]);
}

fn gather() -> Vec<String> {
    [
        "`node ~/.claude/skills/produce/scripts/produce.mjs list --json{repo_flag}` and `status <ID>` of the matching project",
        "existing captures",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[test]
fn refine_asks_for_a_complete_brief_and_lists_the_routes() {
    let meta = route_meta(ROUTED);
    let text = refine_prompt("  اعمل فيلم  ", &meta.routes, &gather(), Some("hogwarts"), Some("## Task\n{{task}}"));
    assert!(text.starts_with("Rewrite the request below as one professional prompt, in English"));
    assert!(text.contains("Fill this template exactly"));
    assert!(text.contains("## Task\n{{task}}"));
    assert!(text.contains("`route: <id>`, one of: plan, do."));
    assert!(text.ends_with("اعمل فيلم"));
    let other = refine_prompt("x", &[], &[], None, None);
    assert!(!other.contains("produce.mjs") && !other.contains("route:"));
    assert!(other.contains("Task · Context · References · Constraints · Deliverable"));
    assert!(valid_job_id(&refine_job_id()));
    assert_ne!(refine_job_id(), refine_job_id());
    assert_eq!(REFINE_SECS, 240);
}

#[test]
fn the_gather_list_comes_from_the_pack_not_from_a_department() {
    // Each entry is one thing to read first, after the department's CLAUDE.md.
    let text = refine_prompt("x", &[], &gather(), Some("hogwarts"), None);
    assert!(text.contains("department's CLAUDE.md; `node ~/.claude/skills/produce/scripts/produce.mjs list --json --repo hogwarts` and `status <ID>` of the matching project; existing captures. Put every reference"));
    // No repo: the token goes without a trace.
    let text = refine_prompt("x", &[], &gather(), None, None);
    assert!(text.contains("produce.mjs list --json` and `status <ID>`"));
    assert!(!text.contains("{repo_flag}") && !text.contains("--repo"));
    // The same department name with another gather list reads that list: nothing is keyed on "video".
    let other = refine_prompt("x", &[], &["read the pricing sheet".to_owned(), "  ".to_owned()], None, None);
    assert!(other.contains("CLAUDE.md; read the pricing sheet. Put every reference"));
    assert!(!other.contains("produce"));
    // With nothing to gather, only CLAUDE.md.
    assert!(refine_prompt("x", &[], &[], None, None).contains("department's CLAUDE.md. Put every reference"));
}

#[test]
fn the_real_video_pack_feeds_refine_its_gather_list() {
    let Ok(kun) = pack::kun_dir() else { return };
    if !kun.join("departments/video/pack.json").is_file() {
        eprintln!("skipped: no kun clone at {}", kun.display());
        return;
    }
    let meta = pack::meta_in(&kun, "video").unwrap();
    let text = refine_prompt("x", &meta.routes, &meta.gather, Some("hogwarts"), None);
    for must in ["produce.mjs list --json --repo hogwarts", "`status <ID>`", "brand-kit.json", "markets.json", "existing captures"] {
        assert!(text.contains(must), "{must}");
    }
}

#[test]
fn the_refined_text_prefers_the_result() {
    assert_eq!(refined_text("streamed ", Some(" final\n")).unwrap(), "final");
    assert_eq!(refined_text(" streamed ", None).unwrap(), "streamed");
    assert_eq!(refined_text("s", Some("  ")).unwrap(), "s");
    assert!(refined_text(" ", None).is_err());
}
