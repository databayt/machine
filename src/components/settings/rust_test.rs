use super::*;

#[test]
fn defaults_when_missing() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(load_in(d.path()).unwrap(), Settings::default());
}

#[test]
fn round_trips_and_trims() {
    let d = tempfile::tempdir().unwrap();
    let saved = save_in(
        d.path(),
        Settings {
            person: "  هويدا ".into(),
            department: "video".into(),
            driver: DriverKind::Opencode,
            login: " moed ".into(),
            auto_run: true,
            ..Settings::default()
        },
    )
    .unwrap();
    assert_eq!(saved.person, "هويدا");
    assert_eq!(saved.login, "moed");
    assert_eq!(load_in(d.path()).unwrap(), saved);
}

#[test]
fn rejects_bad_fields() {
    let d = tempfile::tempdir().unwrap();
    let base = Settings::default();
    assert!(matches!(
        save_in(
            d.path(),
            Settings {
                person: "x".repeat(81),
                ..base.clone()
            }
        ),
        Err(SettingsError::BadPerson)
    ));
    assert!(matches!(
        save_in(
            d.path(),
            Settings {
                person: "a\nb".into(),
                ..base.clone()
            }
        ),
        Err(SettingsError::BadPerson)
    ));
    assert!(matches!(
        save_in(
            d.path(),
            Settings {
                department: "../x".into(),
                ..base
            }
        ),
        Err(SettingsError::BadDepartment)
    ));
    for login in ["Moed", "a b", "../x", "moed_1"] {
        assert!(
            matches!(
                save_in(
                    d.path(),
                    Settings {
                        login: login.into(),
                        ..Settings::default()
                    }
                ),
                Err(SettingsError::BadLogin)
            ),
            "{login}"
        );
    }
    assert!(!d.path().join("settings.json").exists());
}

#[test]
fn partial_file_fills_defaults_and_bad_driver_is_corrupt() {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("settings.json"), r#"{"person":"Ali"}"#).unwrap();
    let s = load_in(d.path()).unwrap();
    assert_eq!((s.person.as_str(), s.driver), ("Ali", DriverKind::Claude));
    fs::write(d.path().join("settings.json"), r#"{"driver":"gpt"}"#).unwrap();
    assert!(matches!(load_in(d.path()), Err(SettingsError::Corrupt(_))));
}

fn profile_of(names: &[&str]) -> MachineProfile {
    use crate::profile::{Access, Department};
    MachineProfile {
        login: "moed".into(),
        name: String::new(),
        os: "macos".into(),
        driver: DriverKind::Claude,
        departments: names
            .iter()
            .map(|n| Department {
                name: (*n).into(),
                title: (*n).into(),
                title_ar: None,
                quick_actions: Vec::new(),
                skills: Vec::new(),
                cards: Vec::new(),
                repos: Vec::new(),
            })
            .collect(),
        department: names.first().map(|n| (*n).to_owned()).unwrap_or_default(),
        repos: Vec::new(),
        access: Access::FULL,
        hidden: Vec::new(),
    }
}

#[test]
fn an_unauthorized_department_is_refused_and_a_stale_one_falls_back() {
    let p = profile_of(&["tech", "sales"]);
    let pick = |d: &str| Settings {
        department: d.into(),
        ..Settings::default()
    };
    assert!(pick("sales").authorized_for(&p).is_ok());
    assert!(pick("").authorized_for(&p).is_ok());
    assert!(matches!(
        pick("video").authorized_for(&p),
        Err(SettingsError::Unauthorized)
    ));
    // What is acted on: an authorized pick, else the registry default.
    assert_eq!(pick("sales").effective(&p).department, "sales");
    assert_eq!(pick("video").effective(&p).department, "tech");
    assert_eq!(pick("").effective(&p).department, "tech");
    // Choosing the default stores nothing; another department is stored.
    assert_eq!(pick("tech").normalized(&p).department, "");
    assert_eq!(pick("sales").normalized(&p).department, "sales");
}

#[test]
fn repo_memory_is_per_department_and_validated() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Settings::default();
    s.repo_by_department.insert("tech".into(), "kun".into());
    let saved = save_in(d.path(), s.clone()).unwrap();
    assert_eq!(load_in(d.path()).unwrap(), saved);
    s.repo_by_department.insert("tech".into(), "../x".into());
    assert!(matches!(
        save_in(d.path(), s),
        Err(SettingsError::BadRepo)
    ));
}

#[test]
fn old_files_load_with_sharing_off_and_share_is_validated() {
    let old = r#"{"person":"x","department":"","repo_by_department":{},"driver":"claude","login":"","autoRun":false,"brain":"claude"}"#;
    let s: Settings = serde_json::from_str(old).unwrap();
    assert_eq!(s.share, ShareSettings { enabled: false, kinds: Vec::new(), idle_minutes: 10 });
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("settings.json"), old).unwrap();
    assert!(!load_in(d.path()).unwrap().share.enabled);
    // The saved shape is the heartbeat's `share`: camelCase.
    let on = Settings { share: ShareSettings { enabled: true, kinds: vec!["encode".into(), "encode".into(), "render".into()], idle_minutes: 5 }, ..Settings::default() };
    let saved = save_in(d.path(), on).unwrap();
    assert_eq!(saved.share.kinds, ["encode", "render"], "duplicates fold");
    assert!(std::fs::read_to_string(d.path().join("settings.json")).unwrap().contains("\"idleMinutes\": 5"));
    for bad in [
        ShareSettings { enabled: true, kinds: vec!["claude".into()], idle_minutes: 10 },
        ShareSettings { enabled: true, kinds: vec![], idle_minutes: 0 },
        ShareSettings { enabled: true, kinds: vec![], idle_minutes: 1441 },
    ] {
        assert!(matches!(save_in(d.path(), Settings { share: bad, ..Settings::default() }), Err(SettingsError::BadShare)));
    }
}

#[test]
fn old_files_read_model_policy_and_fallback_as_automatic_and_both_round_trip() {
    let old = r#"{"person":"x","department":"","repo_by_department":{},"driver":"claude","login":"","autoRun":false,"brain":"claude"}"#;
    let s: Settings = serde_json::from_str(old).unwrap();
    assert_eq!((s.model_policy, s.fallback), (ModelPolicy::Auto, FallbackPolicy::Auto));
    assert_eq!(ModelPolicy::Auto.pinned(), None);
    assert_eq!(ModelPolicy::Opus.pinned(), Some("opus"));
    assert_eq!(ModelPolicy::Sonnet.pinned(), Some("sonnet"));
    let d = tempfile::tempdir().unwrap();
    let on = Settings { model_policy: ModelPolicy::Sonnet, fallback: FallbackPolicy::Ask, ..Settings::default() };
    let saved = save_in(d.path(), on).unwrap();
    let text = std::fs::read_to_string(d.path().join("settings.json")).unwrap();
    assert!(text.contains("\"model_policy\": \"sonnet\"") && text.contains("\"fallback\": \"ask\""));
    assert_eq!(load_in(d.path()).unwrap(), saved);
}
