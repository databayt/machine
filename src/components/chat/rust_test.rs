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

#[test]
fn a_repo_limits_the_run_to_that_repo_and_must_be_known_and_cloned() {
    let root = tempfile::tempdir().unwrap();
    let ws = ws_in(root.path());
    std::fs::create_dir_all(ws.repos.join("kun")).unwrap();
    std::fs::create_dir_all(ws.repos.join("mkan")).unwrap();
    std::fs::create_dir_all(ws.repos.join("secret")).unwrap();
    let r = resolved(DriverKind::Claude, true);
    // Known and cloned.
    assert_eq!(
        validate_repo(&r, "tech", Some("kun"), &ws.repos).unwrap().as_deref(),
        Some("kun")
    );
    // None or empty means all repos.
    assert_eq!(validate_repo(&r, "tech", None, &ws.repos).unwrap(), None);
    assert_eq!(validate_repo(&r, "tech", Some(""), &ws.repos).unwrap(), None);
    // On disk but not one of the profile's repos; or listed but not cloned; or unsafe.
    for bad in ["secret", "hogwarts", "../kun"] {
        assert!(
            matches!(validate_repo(&r, "tech", Some(bad), &ws.repos), Err(ChatError::BadRepo)),
            "{bad}"
        );
    }
    std::fs::remove_dir_all(ws.repos.join("mkan")).unwrap();
    assert!(matches!(
        validate_repo(&r, "tech", Some("mkan"), &ws.repos),
        Err(ChatError::BadRepo)
    ));
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
    assert!(validate_repo(&r, "tech", Some("souq"), root.path()).is_ok());
    assert!(validate_repo(&r, "sales", Some("souq"), root.path()).is_err());
}

#[test]
fn the_chat_repos_are_known_on_every_machine_once_cloned() {
    let root = tempfile::tempdir().unwrap();
    let mut r = resolved(DriverKind::Claude, false);
    r.profile.repos = vec!["kun".into()];
    assert!(matches!(
        validate_repo(&r, "sales", Some("ios-app"), root.path()),
        Err(ChatError::BadRepo)
    ));
    std::fs::create_dir_all(root.path().join("ios-app")).unwrap();
    assert!(validate_repo(&r, "sales", Some("ios-app"), root.path()).is_ok());
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
