use super::*;

fn kun_fixture() -> tempfile::TempDir {
    let kun = tempfile::tempdir().unwrap();
    let dept = kun.path().join("departments/sales");
    fs::create_dir_all(&dept).unwrap();
    fs::write(dept.join("prompt.md"), "You are Sales.\n").unwrap();
    fs::write(
        dept.join("pack.json"),
        r#"{"name":"sales","title":{"ar":"المبيعات","en":"Sales"},"items":["skills/edit","funnel",{"name":"nope"},"../evil"],
            "defaultDriver":"codex",
            "quickActions":[{"label_ar":"عرض","label_en":"Proposal","prompt":"Draft a proposal"},{"label_ar":"x","label_en":"x","prompt":" "}]}"#,
    )
    .unwrap();
    for s in ["edit", "funnel", "unused"] {
        fs::create_dir_all(kun.path().join(".claude/skills").join(s)).unwrap();
    }
    fs::write(kun.path().join("CLAUDE.md"), "# kun").unwrap();
    kun
}

#[test]
fn builds_folder_with_context_and_skill_links() {
    let kun = kun_fixture();
    let home = tempfile::tempdir().unwrap();
    let dir = ensure_in(kun.path(), home.path(), "sales").unwrap();

    let claude = fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with("You are Sales.\n"));
    // Written with forward slashes on every OS, so the @import reads the same on Windows.
    let import = format!("@{}", kun.path().join("CLAUDE.md").display()).replace('\\', "/");
    assert!(claude.contains(&import));
    assert!(
        fs::read_to_string(dir.join("AGENTS.md"))
            .unwrap()
            .contains("Company rules: read")
    );

    let skills = dir.join(".claude/skills");
    assert!(skills.join("edit").exists() && skills.join("funnel").exists());
    assert!(!skills.join("unused").exists());
    assert!(
        !dir.join(".agents").exists(),
        "no .agents/skills in kun, none linked"
    );
}

#[test]
fn is_idempotent_and_prunes_stale_links() {
    let kun = kun_fixture();
    let home = tempfile::tempdir().unwrap();
    let dir = ensure_in(kun.path(), home.path(), "sales").unwrap();
    let stale = dir.join(".claude/skills/unused");
    link::symlink(&kun.path().join(".claude/skills/unused"), &stale).unwrap();
    let mine = dir.join(".claude/skills/mine");
    fs::create_dir_all(&mine).unwrap();

    let again = ensure_in(kun.path(), home.path(), "sales").unwrap();
    assert_eq!(dir, again);
    assert!(
        fs::symlink_metadata(&stale).is_err(),
        "stale link of ours is removed"
    );
    assert!(mine.is_dir(), "real folders are never touched");
    assert!(dir.join(".claude/skills/edit").exists());
}

#[test]
fn rejects_bad_names_and_missing_packs() {
    let kun = kun_fixture();
    let home = tempfile::tempdir().unwrap();
    for bad in ["", "../x", "a/b", ".hidden", "-x"] {
        assert!(
            matches!(
                ensure_in(kun.path(), home.path(), bad),
                Err(PackError::BadName)
            ),
            "{bad}"
        );
    }
    assert!(matches!(
        ensure_in(kun.path(), home.path(), "legal"),
        Err(PackError::Missing(..))
    ));
}

#[test]
fn info_reads_titles_and_drops_empty_quick_actions() {
    let kun = kun_fixture();
    let info = info_in(kun.path(), "sales").unwrap();
    assert_eq!(info.title_ar, "المبيعات");
    assert_eq!(info.default_driver, Some(DriverKind::Codex));
    assert_eq!(info.quick_actions.len(), 1);
    let bare = info_in(kun.path(), "video").unwrap();
    assert_eq!(
        (bare.title_en.as_str(), bare.quick_actions.len()),
        ("video", 0)
    );
}

#[test]
fn lists_departments_with_a_prompt() {
    let kun = kun_fixture();
    fs::create_dir_all(kun.path().join("departments/empty")).unwrap();
    assert_eq!(departments_in(kun.path()), vec!["sales".to_string()]);
}

#[test]
fn item_names_are_filtered() {
    let v: Vec<Value> =
        serde_json::from_str(r#"["skills/edit","..",{"name":"funnel"},3]"#).unwrap();
    assert_eq!(item_names(&v), vec!["edit".to_string(), "funnel".into()]);
}

#[test]
fn catalog_ids_select_only_skills() {
    let v: Vec<Value> =
        serde_json::from_str(r#"["skill:scrape", "agent:lead", "skill:funnel", "pack:sales"]"#)
            .unwrap();
    assert_eq!(item_names(&v), vec!["scrape".to_string(), "funnel".into()]);
}

// ---- workspace ----

use crate::profile::{
    Access, CatalogAccess, CodeAccess, Department, MachineProfile, WorkspaceAccess,
};

fn profile(code: CodeAccess) -> MachineProfile {
    MachineProfile {
        department: String::new(),
        login: "moed".into(),
        name: "معد".into(),
        os: "windows".into(),
        driver: DriverKind::Claude,
        departments: ["sales"]
            .map(|d| Department {
                name: d.into(),
                title: d.into(),
                title_ar: None,
                quick_actions: Vec::new(),
                skills: Vec::new(),
                cards: Vec::new(),
                repos: Vec::new(),

            })
            .into(),
        repos: vec!["kun".into(), "hogwarts".into(), "mkan".into()],
        access: Access {
            code,
            workspace: WorkspaceAccess::Write,
            catalog: CatalogAccess::Profile,
            engine: false,
        },
        hidden: Vec::new(),
    }
}

#[test]
fn read_only_settings_are_pinned() {
    let json = settings_json(
        Path::new("/home/moed/Kun/repos"),
        Path::new("/home/moed/Kun/moed/work"),
        &profile(CodeAccess::Read).access,
    );
    insta::assert_snapshot!("read_only_settings", json);
}

#[test]
fn write_access_has_no_repo_denies_and_no_work_means_no_writes() {
    let repos = Path::new("/r");
    let work = Path::new("/w");
    let write: serde_json::Value =
        serde_json::from_str(&settings_json(repos, work, &Access::FULL)).unwrap();
    assert_eq!(write["permissions"]["deny"].as_array().unwrap().len(), 0);
    let none = Access {
        workspace: WorkspaceAccess::None,
        ..Access::FULL
    };
    let v: serde_json::Value = serde_json::from_str(&settings_json(repos, work, &none)).unwrap();
    assert_eq!(v["permissions"]["allow"].as_array().unwrap().len(), 0);
    assert!(
        v["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "Write(//w/**)")
    );
}

#[test]
fn rule_paths_follow_claude_code_spelling() {
    assert_eq!(
        workspace::rule_path(Path::new("/Users/a/Kun/repos")),
        "//Users/a/Kun/repos"
    );
    assert_eq!(
        workspace::rule_path(Path::new("C:\\Users\\Moed\\Kun\\repos")),
        "//c/Users/Moed/Kun/repos"
    );
}

#[test]
fn the_workspace_has_rules_knowledge_work_and_settings() {
    let kun = kun_fixture();
    let docs = kun.path().join("content/docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(
        docs.join("pricing.mdx"),
        "---\ntitle: Pricing\ndescription: The SAR anchors\n---\nbody",
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let block = root.path().join("repos/hogwarts/src/components/attendance");
    fs::create_dir_all(&block).unwrap();
    fs::write(
        block.join("README.md"),
        "# Attendance\n\nTeachers mark the class in one tap.\nParents get a message.\n",
    )
    .unwrap();
    fs::create_dir_all(root.path().join("repos/mkan")).unwrap();
    fs::write(
        root.path().join("repos/mkan/README.md"),
        "# Mkan\nRentals in Sudan.",
    )
    .unwrap();

    let ws = ensure_workspace_in(kun.path(), root.path(), &profile(CodeAccess::Read)).unwrap();
    assert_eq!(ws.dir, root.path().join("moed"));
    assert!(ws.work.is_dir() && ws.repos.is_dir());

    let claude = fs::read_to_string(ws.dir.join("CLAUDE.md")).unwrap();
    assert!(
        claude.contains("KNOWLEDGE.md")
            && claude.contains("Arabic first")
            && claude.contains("You are Sales.")
    );
    assert!(claude.contains(&ws.work.display().to_string()));
    assert_eq!(
        fs::read_to_string(ws.dir.join("AGENTS.md")).unwrap(),
        claude
    );

    let knowledge = fs::read_to_string(ws.dir.join("KNOWLEDGE.md")).unwrap();
    assert!(knowledge.contains("**Pricing**") && knowledge.contains("The SAR anchors"));
    assert!(
        knowledge.contains("**attendance**")
            && knowledge.contains("Teachers mark the class in one tap.")
    );
    assert!(knowledge.contains("Rentals in Sudan."));
    for step in ["30-second pitch", "Demo path", "Video shot list"] {
        assert!(knowledge.contains(step), "{step}");
    }

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ws.settings_file).unwrap()).unwrap();
    assert!(
        settings["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "Bash(git push*)")
    );
    assert!(
        ws.dir.join(".claude/skills/edit").exists()
            && ws.dir.join(".claude/skills/funnel").exists()
    );

    // idempotent, and `work/` is never touched
    fs::write(ws.work.join("draft.md"), "mine").unwrap();
    let again = ensure_workspace_in(kun.path(), root.path(), &profile(CodeAccess::Read)).unwrap();
    assert_eq!(ws, again);
    assert_eq!(
        fs::read_to_string(ws.work.join("draft.md")).unwrap(),
        "mine"
    );
}

#[test]
fn knowledge_skips_repos_the_profile_does_not_list() {
    let kun = kun_fixture();
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("repos/mkan")).unwrap();
    fs::write(root.path().join("repos/mkan/README.md"), "secret mkan").unwrap();
    let mut p = profile(CodeAccess::Read);
    p.repos = vec!["kun".into()];
    let k = workspace::knowledge_md(kun.path(), &root.path().join("repos"), &p);
    assert!(!k.contains("secret mkan"));
}

#[test]
fn workspace_names_are_checked() {
    let kun = kun_fixture();
    let root = tempfile::tempdir().unwrap();
    let mut p = profile(CodeAccess::Read);
    p.login = "../x".into();
    assert!(matches!(
        ensure_workspace_in(kun.path(), root.path(), &p),
        Err(PackError::BadName)
    ));
}

#[test]
fn copied_skill_folders_are_refreshed_and_pruned() {
    let src = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    fs::create_dir_all(src.path().join("edit")).unwrap();
    fs::write(src.path().join("edit/SKILL.md"), "v1").unwrap();
    let copy = dest.path().join("edit");
    link::copy_dir(&src.path().join("edit"), &copy).unwrap();
    assert!(copy.join(link::COPY_MARKER).is_file());
    fs::write(src.path().join("edit/SKILL.md"), "v2").unwrap();
    link::link_skills(src.path(), dest.path(), &["edit".to_owned()]).unwrap();
    assert_eq!(
        fs::read_to_string(dest.path().join("edit/SKILL.md")).unwrap(),
        "v2"
    );
    link::link_skills(src.path(), dest.path(), &[]).unwrap();
    assert!(
        !dest.path().join("edit").exists(),
        "an unwanted copy of ours is removed"
    );
}

#[test]
fn quick_action_repos_parse_and_default_empty() {
    let kun = kun_fixture();
    let sales = meta_in(kun.path(), "sales").unwrap();
    assert!(sales.quick_actions[0].repos.is_empty());

    let dept = kun.path().join("departments/video");
    fs::create_dir_all(&dept).unwrap();
    fs::write(
        dept.join("pack.json"),
        r#"{"quickActions":[{"label_ar":"a","label_en":"a","prompt":"p","repos":["hogwarts","../evil"]}]}"#,
    )
    .unwrap();
    let video = meta_in(kun.path(), "video").unwrap();
    assert_eq!(video.quick_actions[0].repos, vec!["hogwarts".to_string()]);
}

fn meta_of(json: &str) -> PackMeta {
    let kun = tempfile::tempdir().unwrap();
    let dept = kun.path().join("departments/video");
    fs::create_dir_all(&dept).unwrap();
    fs::write(dept.join("pack.json"), json).unwrap();
    meta_in(kun.path(), "video").unwrap()
}

#[test]
fn dirs_and_timeout_parse_and_default_to_nothing() {
    let m = meta_of(r#"{"dirs":["~/media","/etc"],"timeoutSecs":7200}"#);
    assert_eq!(m.dirs, ["~/media", "/etc"]);
    assert_eq!(m.timeout_secs, Some(7200));
    let none = meta_of(r#"{"name":"video"}"#);
    assert!(none.dirs.is_empty() && none.timeout_secs.is_none());
}

#[test]
fn timeout_is_clamped() {
    assert_eq!(meta_of(r#"{"timeoutSecs":5}"#).timeout_secs, Some(60));
    assert_eq!(meta_of(r#"{"timeoutSecs":999999}"#).timeout_secs, Some(14_400));
}

#[test]
fn pack_dirs_expand_home_and_refuse_escapes() {
    let home = tempfile::tempdir().unwrap();
    let real = home.path().canonicalize().unwrap();
    fs::create_dir_all(real.join("existing")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), real.join("link")).unwrap();
    let meta = PackMeta {
        dirs: [
            "~/media",
            "~/existing",
            "~/existing",
            "/etc",
            "~/../x",
            "relative/dir",
            "~/",
            "~",
            "~/link",
            "~/missing-parent/child",
        ]
        .map(String::from)
        .to_vec(),
        ..PackMeta::default()
    };
    let got = pack_dirs_in(&meta, home.path());
    assert_eq!(got, [real.join("media"), real.join("existing")]);
    assert!(real.join("media").is_dir());
    assert!(!real.join("missing-parent").exists());
}

mod health_tests {
    use super::super::health::*;
    use std::fs;

    fn row(level: &str, critical: bool) -> HealthRow {
        HealthRow { name: "x".into(), level: level.into(), critical, detail: String::new() }
    }

    #[test]
    fn level_computation() {
        assert_eq!(level_of(&[row("ok", true), row("ok", false)]), HealthLevel::Green);
        assert_eq!(level_of(&[row("ok", true), row("warn", true)]), HealthLevel::Orange);
        assert_eq!(level_of(&[row("ok", true), row("fail", false)]), HealthLevel::Orange);
        assert_eq!(level_of(&[row("ok", false), row("fail", true)]), HealthLevel::Red);
        assert_eq!(percent_of(&[row("ok", true), row("warn", false), row("fail", true), row("ok", false)]), 50);
    }

    struct Fx {
        home: tempfile::TempDir,
        kun: tempfile::TempDir,
    }

    fn fx() -> Fx {
        let home = tempfile::tempdir().unwrap();
        let kun = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude/skills/produce/scripts")).unwrap();
        fs::write(home.path().join(".claude/skills/produce/scripts/p.mjs"), "").unwrap();
        fs::create_dir_all(home.path().join("other")).unwrap();
        fs::write(home.path().join("other/p.mjs"), "").unwrap();
        Fx { home, kun }
    }

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn argv_safety() {
        let f = fx();
        let ok = safe_argv(&argv(&["node", "~/.claude/skills/produce/scripts/p.mjs", "readiness", "--json"]), f.home.path(), f.kun.path()).unwrap();
        assert_eq!(ok[0], "node");
        assert!(ok[1].ends_with("produce/scripts/p.mjs"));
        assert_eq!(&ok[2..], ["readiness", "--json"]);

        assert!(safe_argv(&argv(&["/bin/sh", "-c", "id"]), f.home.path(), f.kun.path()).is_err());
        assert!(safe_argv(&argv(&["node", "~/other/p.mjs"]), f.home.path(), f.kun.path()).is_err());
        assert!(safe_argv(&argv(&["node", "~/.claude/skills/../../other/p.mjs"]), f.home.path(), f.kun.path()).is_err());
        assert!(safe_argv(&argv(&["node"]), f.home.path(), f.kun.path()).is_err());
    }

    #[test]
    fn argv_allows_kun_skills_root() {
        let f = fx();
        fs::create_dir_all(f.kun.path().join(".claude/skills/s")).unwrap();
        fs::write(f.kun.path().join(".claude/skills/s/a.mjs"), "").unwrap();
        let p = f.kun.path().join(".claude/skills/s/a.mjs").display().to_string();
        assert!(safe_argv(&argv(&["node", &p]), f.home.path(), f.kun.path()).is_ok());
    }

    #[test]
    fn unparseable_output_is_red() {
        let h = parse_output("not json at all");
        assert_eq!(h.level, HealthLevel::Red);
        assert!(h.error.is_some() && h.rows.is_empty());
        let g = parse_output(r#"{"ok":true,"rows":[{"name":"a","level":"ok","critical":true,"detail":""}]}"#);
        assert_eq!((g.level, g.percent), (HealthLevel::Green, 100));
        let noisy = parse_output("log line\n{\"ok\":false,\"rows\":[{\"name\":\"a\",\"level\":\"warn\"}]}");
        assert_eq!(noisy.level, HealthLevel::Orange);
    }

    #[tokio::test]
    async fn no_health_is_none_and_bad_argv_is_red() {
        let f = fx();
        assert_eq!(evaluate(None, f.home.path(), f.kun.path(), false).await.level, HealthLevel::None);
        let spec = HealthSpec { check: argv(&["/bin/sh", "-c", "echo"]), fix: vec![] };
        let h = evaluate(Some(&spec), f.home.path(), f.kun.path(), false).await;
        assert_eq!(h.level, HealthLevel::Red);
        assert!(h.error.is_some());
    }
}

#[test]
fn run_block_parses_and_drops_invalid_fields() {
    let m = meta_of(
        r#"{"run":{"plan":{"model":"opus","effort":"high","agent":"producer"},
            "work":{"model":"claude-sonnet-4-6","effort":"ultra","agent":"../x"}}}"#,
    );
    let run = m.run.unwrap();
    assert_eq!(run.plan.unwrap().effort.as_deref(), Some("high"));
    let work = run.work.unwrap();
    assert_eq!(work.model.as_deref(), Some("claude-sonnet-4-6"));
    assert_eq!((work.effort, work.agent), (None, None));
    assert!(meta_of("{}").run.is_none());
}

#[test]
fn fable_and_unknown_models_are_rejected() {
    for bad in ["fable", "claude-opus-fable-1", "FABLE", "gpt-5", "claude-opus", "opus 4"] {
        let m = meta_of(&format!(r#"{{"run":{{"plan":{{"model":"{bad}"}}}}}}"#));
        assert!(m.run.is_none(), "{bad}");
    }
}

fn v(s: &[&str]) -> Vec<String> {
    s.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn routes_parse_with_snake_or_camel_fields() {
    let m = meta_of(
        r#"{"defaultRoute":"do","routes":[
            {"id":"plan","label_ar":"خطة","label_en":"Plan","mode":"plan","model":"opus","effort":"high","agent":"producer","suffix":"  Ask first.  "},
            {"id":"do","labelEn":"Do","descriptionEn":"Now","icon":"zap","mode":"auto"}]}"#,
    );
    assert_eq!(m.routes.len(), 2);
    let plan = &m.routes[0];
    assert_eq!(
        (plan.mode.as_str(), plan.suffix.as_str(), plan.label_ar.as_str()),
        ("plan", "Ask first.", "خطة")
    );
    assert_eq!(route_flags(plan), v(&["--model", "opus", "--effort", "high", "--agent", "producer"]));
    let d = &m.routes[1];
    assert_eq!((d.label_en.as_str(), d.description_en.as_str(), d.icon.as_deref()), ("Do", "Now", Some("zap")));
    assert_eq!(d.label_ar, "do"); // no label: the id shows
    assert!(d.suffix.is_empty() && route_flags(d).is_empty());
    assert_eq!(m.default_route.as_deref(), Some("do"));
}

#[test]
fn invalid_routes_are_skipped_not_fatal() {
    let long = "x".repeat(401);
    let json = format!(
        r#"{{"routes":[
        {{"id":"fab","mode":"plan","model":"fable"}},
        {{"id":"eff","mode":"plan","effort":"ultra"}},
        {{"id":"agt","mode":"plan","agent":"../x"}},
        {{"id":"Bad Id","mode":"plan"}},
        {{"id":"mode","mode":"yolo"}},
        {{"id":"nomode"}},
        {{"id":"long","mode":"plan","suffix":"{long}"}},
        {{"id":"edge","mode":"auto","suffix":"{ok}"}},
        {{"id":"edge","mode":"plan"}},
        "not an object",
        {{"id":"ok","mode":"plan","model":"claude-sonnet-4-6"}}]}}"#,
        ok = "y".repeat(400)
    );
    let m = meta_of(&json);
    let ids: Vec<_> = m.routes.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["edge", "ok"]); // exactly 400 chars is kept; the duplicate id is dropped
    assert_eq!(m.default_route.as_deref(), Some("edge"));
}

#[test]
fn at_most_six_routes() {
    let routes: Vec<String> = (0..9).map(|i| format!(r#"{{"id":"r{i}","mode":"auto"}}"#)).collect();
    let m = meta_of(&format!(r#"{{"routes":[{}]}}"#, routes.join(",")));
    assert_eq!(m.routes.len(), MAX_ROUTES);
}

#[test]
fn a_run_block_without_routes_derives_plan_and_do() {
    let m = meta_of(
        r#"{"run":{"plan":{"model":"opus","effort":"high","agent":"producer"},"work":{"effort":"medium"}}}"#,
    );
    let ids: Vec<_> = m.routes.iter().map(|r| (r.id.as_str(), r.mode.as_str())).collect();
    assert_eq!(ids, [("plan", "plan"), ("do", "auto")]);
    assert_eq!(m.default_route.as_deref(), Some("plan"));
    assert_eq!(m.routes[0].label_en, "Plan first");
    assert_eq!(m.routes[1].label_ar, "نفّذ الآن");
    assert_eq!(
        route_flags(&m.routes[0]),
        v(&["--model", "opus", "--effort", "high", "--agent", "producer"])
    );
    assert_eq!(route_flags(&m.routes[1]), v(&["--effort", "medium"]));
    // Declared routes win over `run`.
    let m = meta_of(r#"{"run":{"plan":{"model":"opus"}},"routes":[{"id":"x","mode":"auto"}]}"#);
    assert_eq!(m.routes.len(), 1);
    let none = meta_of("{}");
    assert!(none.routes.is_empty() && none.default_route.is_none());
}

#[test]
fn default_route_must_exist_else_the_first() {
    let m = meta_of(r#"{"defaultRoute":"ghost","routes":[{"id":"a","mode":"auto"},{"id":"b","mode":"plan"}]}"#);
    assert_eq!(m.default_route.as_deref(), Some("a"));
    // A default naming a dropped route falls back too.
    let m = meta_of(
        r#"{"defaultRoute":"bad","routes":[{"id":"bad","mode":"plan","model":"fable"},{"id":"b","mode":"plan"}]}"#,
    );
    assert_eq!(m.default_route.as_deref(), Some("b"));
}

#[test]
fn find_route_defaults_and_refuses_unknown_ids() {
    let m = meta_of(r#"{"defaultRoute":"b","routes":[{"id":"a","mode":"auto"},{"id":"b","mode":"plan"}]}"#);
    assert_eq!(find_route(&m, None).unwrap().unwrap().id, "b");
    assert_eq!(find_route(&m, Some("a")).unwrap().unwrap().id, "a");
    assert!(matches!(find_route(&m, Some("zzz")), Err(PackError::UnknownRoute(id)) if id == "zzz"));
    assert!(find_route(&meta_of("{}"), None).unwrap().is_none());
}

#[test]
fn quick_action_routes_and_info_expose_routes() {
    let kun = tempfile::tempdir().unwrap();
    let dept = kun.path().join("departments/video");
    fs::create_dir_all(&dept).unwrap();
    fs::write(dept.join("prompt.md"), "p").unwrap();
    fs::write(
        dept.join("pack.json"),
        r#"{"routes":[{"id":"a","mode":"plan"}],"quickActions":[
            {"label_en":"x","prompt":"p","route":"a"},{"label_en":"y","prompt":"q","route":"../bad"},{"label_en":"z","prompt":"r"}]}"#,
    )
    .unwrap();
    let info = info_in(kun.path(), "video").unwrap();
    assert_eq!(info.default_route.as_deref(), Some("a"));
    assert_eq!(info.routes[0].id, "a");
    let routes: Vec<_> = info.quick_actions.iter().map(|q| q.route.as_deref()).collect();
    assert_eq!(routes, [Some("a"), None, None]);
}

#[test]
fn route_flags_pass_the_driver_validation() {
    let m = meta_of(
        r#"{"run":{"plan":{"model":"opus","effort":"high","agent":"producer"}}}"#,
    );
    let job = crate::driver::Job {
        job_id: "j".into(),
        driver: DriverKind::Claude,
        prompt: "hi".into(),
        cwd: "/".into(),
        session: Some("s-1".into()),
        timeout_secs: None,
        extra_args: route_flags(&m.routes[0]),
        env: vec![],
        permission_mode: m.routes[0].permission(),
    };
    let args = crate::driver::args_for(&job).unwrap();
    assert!(args.windows(2).any(|w| w == ["--permission-mode", "plan"]));
    assert!(args.windows(2).any(|w| w == ["--agent", "producer"]));
    assert!(args.windows(2).any(|w| w == ["--effort", "high"]));
}

#[test]
fn agents_link_into_the_workdir_and_stale_links_go() {
    let kun = tempfile::tempdir().unwrap();
    let dept = kun.path().join("departments/video");
    fs::create_dir_all(&dept).unwrap();
    fs::write(dept.join("prompt.md"), "p").unwrap();
    let write_pack = |items: &str| {
        fs::write(dept.join("pack.json"), format!(r#"{{"items":{items}}}"#)).unwrap();
    };
    let agents = kun.path().join(".claude/agents");
    fs::create_dir_all(&agents).unwrap();
    for a in ["media-qa", "producer"] {
        fs::write(agents.join(format!("{a}.md")), "a").unwrap();
    }
    let machine = tempfile::tempdir().unwrap();
    write_pack(r#"["agent:media-qa","agent:producer","agent:ghost","edit"]"#);
    let dir = ensure_in(kun.path(), machine.path(), "video").unwrap();
    let out = dir.join(".claude/agents");
    assert!(out.join("media-qa.md").exists() && out.join("producer.md").exists());
    assert!(!out.join("ghost.md").exists());
    fs::write(out.join("mine.md"), "real").unwrap();
    write_pack(r#"["agent:producer"]"#);
    ensure_in(kun.path(), machine.path(), "video").unwrap();
    assert!(!out.join("media-qa.md").exists());
    assert!(out.join("producer.md").exists());
    assert!(out.join("mine.md").exists());
}
