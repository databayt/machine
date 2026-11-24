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
fn a_lane_becomes_a_login_interactive_zsh() {
    let home = tempfile::tempdir().unwrap();
    let p = plan("c", &specs(), None, home.path()).unwrap();
    assert_eq!(p.argv[..4], ["/bin/zsh", "-l", "-i", "-c"]);
    assert_eq!(p.argv[4], "exec 'claude' '--dangerously-skip-permissions'");
    assert_eq!(p.argv.len(), 5);
    assert_eq!(plan("f", &specs(), None, home.path()).unwrap().argv[4], "exec 'kun-free.sh'");
}

#[test]
fn only_a_listed_lane_runs() {
    let home = tempfile::tempdir().unwrap();
    for lane in ["", "x", "claude", "c; rm -rf ~", "C", "c "] {
        assert!(matches!(plan(lane, &specs(), None, home.path()), Err(TerminalError::Unknown(_))), "{lane:?}");
    }
}

#[test]
fn fable_is_refused_wherever_it_hides() {
    let home = tempfile::tempdir().unwrap();
    let bad = vec![
        spec("a", "claude", &["--model", "Claude-Fable-5"]),
        spec("b", "fable-cli", &[]),
    ];
    assert!(matches!(plan("a", &bad, None, home.path()), Err(TerminalError::Refused(_))));
    assert!(matches!(plan("b", &bad, None, home.path()), Err(TerminalError::Refused(_))));
    let dir = home.path().join("fable-dir");
    std::fs::create_dir(&dir).unwrap();
    assert!(matches!(
        plan("c", &specs(), dir.to_str(), home.path()),
        Err(TerminalError::Refused(_))
    ));
}

#[test]
fn the_folder_defaults_to_kun_then_home_and_must_exist() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(plan("c", &specs(), None, home.path()).unwrap().cwd, home.path());
    std::fs::create_dir(home.path().join("kun")).unwrap();
    assert_eq!(plan("c", &specs(), Some("  "), home.path()).unwrap().cwd, home.path().join("kun"));
    let work = tempfile::tempdir().unwrap();
    assert_eq!(plan("c", &specs(), work.path().to_str(), home.path()).unwrap().cwd, work.path());
    for bad in ["relative/dir", "/definitely/not/here"] {
        assert!(matches!(plan("c", &specs(), Some(bad), home.path()), Err(TerminalError::Refused(_))), "{bad}");
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
