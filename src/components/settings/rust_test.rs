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
