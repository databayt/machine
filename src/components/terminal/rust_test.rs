use super::*;

fn spec(id: &str, bin: &str, args: &[&str]) -> LauncherSpec {
    LauncherSpec {
        id: id.into(),
        name: id.into(),
        bin: bin.into(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    }
}

fn specs() -> Vec<LauncherSpec> {
    vec![
        spec("c", "claude", &["--dangerously-skip-permissions"]),
        spec("f", "kun-free.sh", &[]),
        spec("o", "opencode", &[]),
    ]
}

#[test]
fn quoting_survives_embedded_quotes() {
    assert_eq!(shell_quote("a b"), "'a b'");
    assert_eq!(shell_quote("it's"), r"'it'\''s'");
    assert_eq!(shell_quote(""), "''");
}

#[test]
fn a_launcher_runs_inside_a_shell_that_survives_it() {
    let home = tempfile::tempdir().unwrap();
    let p = plan(Some("c"), &specs(), None, home.path()).unwrap();
    assert_eq!(p.argv[..4], ["/bin/zsh", "-l", "-i", "-c"]);
    assert_eq!(p.argv[4], "'claude' '--dangerously-skip-permissions'; exec /bin/zsh -l -i");
    assert_eq!(p.argv.len(), 5);
    assert_eq!((p.lane.as_str(), p.base.as_str()), ("c", "c"));
    assert_eq!(plan(Some("f"), &specs(), None, home.path()).unwrap().argv[4], "'kun-free.sh'; exec /bin/zsh -l -i");
}

#[test]
fn local_is_a_plain_login_shell() {
    let home = tempfile::tempdir().unwrap();
    for lane in [None, Some("local"), Some(" local "), Some("")] {
        let p = plan(lane, &specs(), None, home.path()).unwrap();
        assert_eq!(p.argv, ["/bin/zsh", "-l", "-i"], "{lane:?}");
        assert_eq!((p.lane.as_str(), p.base.as_str()), ("local", "Local"));
        assert_eq!(p.cwd, home.path());
    }
}

#[test]
fn only_a_listed_lane_runs() {
    let home = tempfile::tempdir().unwrap();
    for lane in ["x", "claude", "c; rm -rf ~", "C", "LOCAL"] {
        assert!(matches!(plan(Some(lane), &specs(), None, home.path()), Err(TerminalError::Unknown(_))), "{lane:?}");
    }
}

#[test]
fn fable_is_refused_wherever_it_hides() {
    let home = tempfile::tempdir().unwrap();
    let bad = vec![
        spec("a", "claude", &["--model", "Claude-Fable-5"]),
        spec("b", "fable-cli", &[]),
    ];
    assert!(matches!(plan(Some("a"), &bad, None, home.path()), Err(TerminalError::Refused(_))));
    assert!(matches!(plan(Some("b"), &bad, None, home.path()), Err(TerminalError::Refused(_))));
    let dir = home.path().join("fable-dir");
    std::fs::create_dir(&dir).unwrap();
    assert!(matches!(plan(Some("c"), &specs(), dir.to_str(), home.path()), Err(TerminalError::Refused(_))));
    assert!(matches!(plan(None, &specs(), dir.to_str(), home.path()), Err(TerminalError::Refused(_))));
}

#[test]
fn the_folder_defaults_to_kun_then_home_and_must_exist() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(plan(Some("c"), &specs(), None, home.path()).unwrap().cwd, home.path());
    std::fs::create_dir(home.path().join("kun")).unwrap();
    assert_eq!(plan(Some("c"), &specs(), Some("  "), home.path()).unwrap().cwd, home.path().join("kun"));
    let work = tempfile::tempdir().unwrap();
    assert_eq!(plan(None, &specs(), work.path().to_str(), home.path()).unwrap().cwd, work.path());
    for bad in ["relative/dir", "/definitely/not/here"] {
        assert!(matches!(plan(Some("c"), &specs(), Some(bad), home.path()), Err(TerminalError::Refused(_))), "{bad}");
    }
}

#[test]
fn the_grid_is_clamped() {
    assert_eq!((size(0, 0).cols, size(0, 0).rows), (1, 1));
    assert_eq!(size(9999, 80).cols, MAX_DIM);
}

#[test]
fn page_chosen_ids_are_plain() {
    assert!(valid_id("t1") && valid_id("a-b_C9"));
    for bad in ["", "a/b", "a b", "a:b", "..", &"x".repeat(41)] {
        assert!(!valid_id(bad), "{bad:?}");
    }
}

fn local(base: &str, cwd: &str) -> Plan {
    Plan { argv: vec![], cwd: PathBuf::from(cwd), lane: "local".into(), base: base.into() }
}

fn names(r: &Registry) -> Vec<String> {
    r.list().sessions.into_iter().map(|s| s.name).collect()
}

#[test]
fn the_ring_keeps_only_the_newest_bytes_and_counts_all() {
    let mut r = Ring::new(8);
    assert_eq!(r.push(b"abcde"), 5);
    assert_eq!(r.snapshot(), b"abcde");
    assert_eq!(r.push(b"fghij"), 10);
    assert_eq!(r.snapshot(), b"cdefghij");
    // One chunk bigger than the whole ring keeps just its tail.
    assert_eq!(r.push(b"0123456789AB"), 22);
    assert_eq!(r.snapshot(), b"456789AB");
    assert_eq!(r.total(), 22);
    assert_eq!(r.snapshot().len(), 8);
}

#[test]
fn the_real_ring_is_512_kb() {
    let mut r = Ring::new(SCROLLBACK_BYTES);
    for _ in 0..200 {
        r.push(&[b'x'; 8192]);
    }
    assert_eq!(r.snapshot().len(), 512 * 1024);
    assert_eq!(r.total(), 200 * 8192);
}

#[test]
fn local_shells_are_numbered_like_jetbrains() {
    let mut r = Registry::default();
    for _ in 0..3 {
        let id = r.fresh_id();
        r.add(id, &local("Local", "/tmp"), None);
    }
    assert_eq!(names(&r), ["Local", "Local (2)", "Local (3)"]);
    // The first free number is reused.
    r.remove("t2");
    let id = r.fresh_id();
    r.add(id, &local("Local", "/tmp"), None);
    assert_eq!(names(&r), ["Local", "Local (3)", "Local (2)"]);
    assert_eq!(r.unique_name("hermes"), "hermes");
}

#[test]
fn rename_trims_caps_and_falls_back() {
    let mut r = Registry::default();
    r.add("a".into(), &local("Local", "/tmp"), None);
    r.add("b".into(), &local("Local", "/tmp"), None);
    r.rename("a", "  build \u{7}box  ").unwrap();
    assert_eq!(r.list().sessions[0].name, "build box");
    r.rename("a", &"é".repeat(60)).unwrap();
    assert_eq!(r.list().sessions[0].name.chars().count(), 40);
    r.rename("a", "   ").unwrap();
    assert_eq!(r.list().sessions[0].name, "Local");
    r.rename("b", "").unwrap();
    assert_eq!(r.list().sessions[1].name, "Local (2)");
    assert!(matches!(r.rename("zz", "x"), Err(TerminalError::Gone)));
}

#[test]
fn close_hands_the_front_to_a_neighbour() {
    let mut r = Registry::default();
    for id in ["a", "b", "c", "d"] {
        r.add(id.into(), &local("Local", "/tmp"), None);
    }
    assert_eq!(r.list().active.as_deref(), Some("d"));
    r.set_active("b").unwrap();
    r.remove("c");
    assert_eq!(r.list().active.as_deref(), Some("b"));
    r.remove("b");
    assert_eq!(r.list().active.as_deref(), Some("a"));
    r.remove("a");
    assert_eq!(r.list().active.as_deref(), Some("d"));
    r.remove("d");
    assert_eq!(r.list(), TerminalList::default());
    assert!(r.remove("d").is_none());
    assert!(matches!(r.set_active("d"), Err(TerminalError::Gone)));
}

#[test]
fn close_others_keeps_one_in_front() {
    let mut r = Registry::default();
    for id in ["a", "b", "c"] {
        r.add(id.into(), &local("Local", "/tmp"), None);
    }
    let gone = r.remove_others("b").unwrap();
    assert_eq!(gone.len(), 2);
    let l = r.list();
    assert_eq!(l.sessions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["b"]);
    assert_eq!(l.active.as_deref(), Some("b"));
    assert!(matches!(r.remove_others("nope"), Err(TerminalError::Gone)));
    assert_eq!(r.len(), 1);
}

#[test]
fn list_keeps_open_order_and_a_new_session_takes_the_front() {
    let mut r = Registry::default();
    r.add("a".into(), &local("Local", "/tmp"), None);
    r.add("b".into(), &local("hermes", "/tmp"), None);
    r.set_active("a").unwrap();
    r.add("c".into(), &local("Local", "/tmp"), None);
    let l = r.list();
    assert_eq!(l.sessions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(l.active.as_deref(), Some("c"));
    assert!(l.sessions.iter().all(|s| !s.alive && s.created_at > 0));
}

#[test]
fn attach_replays_what_was_pushed_and_says_how_far() {
    let mut r = Registry::default();
    r.add("a".into(), &local("Local", "/tmp"), None);
    assert_eq!(r.push("a", b"hello "), Some(6));
    assert_eq!(r.push("a", b"world"), Some(11));
    assert_eq!(r.attach("a").unwrap(), (b"hello world".to_vec(), 11));
    assert_eq!(r.push("zz", b"x"), None);
    assert!(matches!(r.attach("zz"), Err(TerminalError::Gone)));
}

#[test]
fn a_dead_session_stays_listed_but_not_alive() {
    let mut r = Registry::default();
    r.add("a".into(), &local("Local", "/tmp"), None);
    r.mark_dead("a");
    assert!(!r.list().sessions[0].alive);
    assert_eq!(r.len(), 1);
}

#[test]
fn clean_name_rules() {
    assert_eq!(clean_name("  hi\n  "), "hi");
    assert_eq!(clean_name(""), "");
    assert_eq!(clean_name(&"x".repeat(41)).len(), 40);
}
