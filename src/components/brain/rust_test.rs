use std::{cell::RefCell, collections::HashMap};

use super::*;

fn s(env: &[(&str, &str)]) -> Vec<(String, String)> {
    env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

#[test]
fn editor_updates_in_place_and_keeps_everything_else() {
    let before = "# my config\nexport NVIDIA_NIM_API_KEY=old\n\nPORT=9000\nMODEL=\"x\"\n";
    let after = edit_env(before, &[("NVIDIA_NIM_API_KEY", "nvapi-abc"), ("MODEL", "nvidia_nim/m")]);
    assert_eq!(after, "# my config\nNVIDIA_NIM_API_KEY=nvapi-abc\n\nPORT=9000\nMODEL=nvidia_nim/m\n");
}

#[test]
fn editor_inserts_missing_keys_and_creates_from_empty() {
    assert_eq!(
        edit_env("", &[("OPENROUTER_API_KEY", "k"), ("MODEL", "open_router/openrouter/free")]),
        "OPENROUTER_API_KEY=k\nMODEL=open_router/openrouter/free\n"
    );
    assert_eq!(edit_env("A=1", &[("B", "2")]), "A=1\nB=2\n");
}

#[test]
fn editor_ignores_comments_drops_duplicates_and_quotes_odd_values() {
    let out = edit_env("# MODEL=nope\nMODEL=a\nMODEL=b\n", &[("MODEL", "c")]);
    assert_eq!(out, "# MODEL=nope\nMODEL=c\n");
    assert_eq!(edit_env("", &[("K", "a b\"c")]), "K=\"a b\\\"c\"\n");
    // Idempotent: applying twice changes nothing.
    let twice = edit_env(&out, &[("MODEL", "c")]);
    assert_eq!(twice, out);
}

#[test]
fn apply_writes_a_private_file_and_reports_change() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join(".fcc/.env");
    assert!(apply_fcc_env(&path, Brain::Nvidia, "nvapi-1").unwrap());
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("NVIDIA_NIM_API_KEY=nvapi-1") && text.contains(&format!("MODEL={NVIDIA_MODEL}")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert!(!apply_fcc_env(&path, Brain::Nvidia, "nvapi-1").unwrap(), "same content, no change");
    // Switching route updates MODEL and keeps the other key.
    assert!(apply_fcc_env(&path, Brain::OpenRouter, "or-1").unwrap());
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("NVIDIA_NIM_API_KEY=nvapi-1") && text.contains("OPENROUTER_API_KEY=or-1"));
    assert!(text.contains(&format!("MODEL={OPENROUTER_MODEL}")) && !text.contains(NVIDIA_MODEL));
    // OmniRoute and Claude never touch the file.
    assert!(!apply_fcc_env(&path, Brain::OmniRoute, "k").unwrap());
}

#[test]
fn old_settings_files_load_as_claude() {
    let old = r#"{"person":"x","department":"","repo_by_department":{},"driver":"claude","login":"","autoRun":false}"#;
    let s: settings::Settings = serde_json::from_str(old).unwrap();
    assert_eq!(s.brain, Brain::Claude);
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("settings.json"), old).unwrap();
    assert_eq!(settings::load_in(d.path()).unwrap().brain, Brain::Claude);
    // And a chosen route round-trips by its lowercase name.
    let saved = settings::save_in(d.path(), settings::Settings { brain: Brain::OpenRouter, ..settings::Settings::default() }).unwrap();
    assert!(fs::read_to_string(d.path().join("settings.json")).unwrap().contains("\"brain\": \"openrouter\""));
    assert_eq!(settings::load_in(d.path()).unwrap(), saved);
}

#[test]
fn the_env_per_route() {
    assert!(route_env(Brain::Claude, None).is_empty());
    assert_eq!(
        route_env(Brain::Nvidia, None),
        s(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8082"), ("ANTHROPIC_AUTH_TOKEN", "freecc")])
    );
    assert_eq!(route_env(Brain::OpenRouter, Some("ignored")), route_env(Brain::Nvidia, None));
    assert_eq!(
        route_env(Brain::OmniRoute, Some("sk-omni")),
        s(&[
            ("ANTHROPIC_BASE_URL", "http://localhost:20128"),
            ("ANTHROPIC_AUTH_TOKEN", "sk-omni"),
            ("ANTHROPIC_MODEL", "auto"),
        ])
    );
}

#[test]
fn merging_strips_other_anthropic_vars_and_the_telemetry_switch() {
    let base = s(&[
        ("TWENTY_API_KEY", "t"),
        ("ANTHROPIC_API_KEY", "sk-ant"),
        ("ANTHROPIC_MODEL", "opus"),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ]);
    for brain in [Brain::Nvidia, Brain::OpenRouter, Brain::OmniRoute] {
        let out = merge_env(base.clone(), route_env(brain, Some("k")));
        assert!(out.contains(&("TWENTY_API_KEY".into(), "t".into())), "{brain:?}");
        assert!(!out.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY" || k == "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"));
        assert_eq!(out.iter().filter(|(k, _)| k == "ANTHROPIC_BASE_URL").count(), 1);
        assert_eq!(out.iter().filter(|(k, _)| k == "ANTHROPIC_MODEL").count(), usize::from(brain == Brain::OmniRoute));
    }
    // Claude leaves the environment exactly as it was.
    assert_eq!(merge_env(base.clone(), route_env(Brain::Claude, None)), base);
}

#[test]
fn builtin_secrets_are_allowed_for_every_profile_and_nothing_else_is() {
    #[derive(Default)]
    struct Mem(RefCell<HashMap<String, String>>);
    impl SecretStore for Mem {
        fn set(&self, a: &str, v: &str) -> Result<(), String> {
            self.0.borrow_mut().insert(a.into(), v.into());
            Ok(())
        }
        fn get(&self, a: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(a).cloned())
        }
    }
    let resolved = profile::resolve_in(Path::new("/nonexistent-kun"), "", crate::driver::DriverKind::Claude).unwrap();
    let store = Mem::default();
    for name in profile::BUILTIN_SECRETS {
        profile::store_secret(&store, &resolved, name, " k\n").unwrap();
        assert!(profile::secret_present(&store, &resolved, name).unwrap());
        assert_eq!(profile::read_secret(&store, &resolved, name).unwrap().as_deref(), Some("k"));
    }
    assert!(matches!(
        profile::store_secret(&store, &resolved, "ANTHROPIC_API_KEY", "k"),
        Err(ProfileError::NotAllowed(_))
    ));
}
