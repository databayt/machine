use super::*;

fn events(kind: DriverKind, fixture: &str) -> Vec<Event> {
    fixture.lines().flat_map(|l| parse_line(kind, l)).collect()
}

fn job(driver: DriverKind, prompt: &str, session: Option<&str>) -> Job {
    Job {
        job_id: "j1".into(),
        driver,
        prompt: prompt.into(),
        cwd: ".".into(),
        session: session.map(str::to_owned),
        timeout_secs: None,
        extra_args: Vec::new(),
        env: Vec::new(),
        permission_mode: PermissionMode::Auto,
        seat: None,
    }
}

#[test]
fn claude_stream_normalizes() {
    let got = events(DriverKind::Claude, include_str!("fixtures/claude.jsonl"));
    let session = "11111111-2222-3333-4444-555555555555".to_string();
    assert_eq!(
        got,
        vec![
            Event::Started {
                session: session.clone()
            },
            Event::Text {
                text: "Drafting the proposal.".into()
            },
            Event::Tool {
                name: "Read".into()
            },
            Event::Text {
                text: "عرض السعر جاهز.".into()
            },
            Event::Done {
                session: Some(session),
                ok: true,
                result: Some("عرض السعر جاهز.".into())
            },
        ]
    );
}

#[test]
fn codex_stream_normalizes() {
    let got = events(DriverKind::Codex, include_str!("fixtures/codex.jsonl"));
    assert_eq!(
        got,
        vec![
            Event::Started {
                session: "thr_fixture".into()
            },
            Event::Tool {
                name: "shell".into()
            },
            Event::Text {
                text: "Done.".into()
            },
            Event::Done {
                session: None,
                ok: true,
                result: None
            },
        ]
    );
}

#[test]
fn opencode_error_surfaces_message() {
    let got = events(
        DriverKind::Opencode,
        include_str!("fixtures/opencode-error.jsonl"),
    );
    assert_eq!(
        got,
        vec![Event::Error {
            message: "This API key has expired.".into()
        }]
    );
}

#[test]
fn garbage_lines_are_ignored() {
    for kind in DriverKind::ALL {
        assert!(parse_line(kind, "not json").is_empty());
        assert!(parse_line(kind, "{}").is_empty());
    }
}

#[test]
fn claude_args_end_options_before_prompt() {
    let args = args_for(&job(DriverKind::Claude, "--dangerous", Some("abc-123"))).unwrap();
    let dashdash = args.iter().position(|a| a == "--").unwrap();
    assert_eq!(args[dashdash + 1], "--dangerous");
    assert!(args.windows(2).any(|w| w == ["--resume", "abc-123"]));
    assert!(args.windows(2).any(|w| w == ["--permission-mode", "auto"]));
}

#[test]
fn permission_mode_allowlist_and_claude_arg_vector() {
    assert_eq!(PermissionMode::parse(Some("plan")), PermissionMode::Plan);
    assert_eq!(PermissionMode::parse(Some("auto")), PermissionMode::Auto);
    for other in [None, Some(""), Some("PLAN"), Some("bypassPermissions"), Some("plan ")] {
        assert_eq!(PermissionMode::parse(other), PermissionMode::Auto, "{other:?}");
    }
    let plan = Job { permission_mode: PermissionMode::Plan, ..job(DriverKind::Claude, "hi", None) };
    let args = args_for(&plan).unwrap();
    assert!(args.windows(2).any(|w| w == ["--permission-mode", "plan"]));
    assert!(!args.iter().any(|a| a == "auto"));
    // The other drivers ignore it.
    for kind in [DriverKind::Codex, DriverKind::Opencode, DriverKind::Agy, DriverKind::Hermes] {
        let j = Job { permission_mode: PermissionMode::Plan, ..job(kind, "hi", None) };
        assert_eq!(args_for(&j).unwrap(), args_for(&job(kind, "hi", None)).unwrap(), "{kind:?}");
    }
}

#[test]
fn codex_resume_uses_subcommand() {
    let args = args_for(&job(DriverKind::Codex, "hi", Some("thr_1"))).unwrap();
    assert_eq!(
        &args[..4],
        ["exec", "--json", "--sandbox", "workspace-write"]
    );
    assert!(args.windows(2).any(|w| w == ["resume", "thr_1"]));
}

#[test]
fn invalid_jobs_are_refused() {
    assert!(args_for(&job(DriverKind::Claude, "   ", None)).is_err());
    assert!(args_for(&job(DriverKind::Claude, "hi", Some("x; rm -rf /"))).is_err());
    assert!(
        args_for(&job(
            DriverKind::Claude,
            &"a".repeat(MAX_PROMPT_BYTES + 1),
            None
        ))
        .is_err()
    );
}

#[test]
fn detect_lists_every_driver() {
    let got = detect();
    assert_eq!(got.len(), DriverKind::ALL.len());
}

fn with_extras(extra: &[&str], env: &[(&str, &str)]) -> Job {
    Job {
        extra_args: extra.iter().map(|s| (*s).to_owned()).collect(),
        env: env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        ..job(DriverKind::Claude, "hi", Some("s-1"))
    }
}

#[test]
fn claude_extras_sit_before_resume_and_the_prompt() {
    let j = with_extras(
        &[
            "--add-dir",
            "/r/kun",
            "--add-dir",
            "/r/mkan",
            "--settings",
            "/w/.claude-settings.json",
        ],
        &[],
    );
    let args = args_for(&j).unwrap();
    let pos = |needle: &str| args.iter().position(|a| a == needle).unwrap();
    assert!(args.windows(2).any(|w| w == ["--add-dir", "/r/kun"]));
    assert!(
        args.windows(2)
            .any(|w| w == ["--settings", "/w/.claude-settings.json"])
    );
    assert!(pos("--settings") < pos("--resume") && pos("--resume") < pos("--"));
    assert_eq!(args.last().map(String::as_str), Some("hi"));
}

#[test]
fn extras_are_validated() {
    for bad in [
        &["--dangerously-skip-permissions", "x"][..],
        &["--add-dir"],
        &["--add-dir", "--settings"],
        &["--add-dir", ""],
        &["--", " x"],
    ] {
        assert!(args_for(&with_extras(bad, &[])).is_err(), "{bad:?}");
    }
    let mut codex = with_extras(&["--add-dir", "/r"], &[]);
    codex.driver = DriverKind::Codex;
    assert!(args_for(&codex).is_err());
}

#[test]
fn env_is_validated() {
    assert!(args_for(&with_extras(&[], &[("TWENTY_API_KEY", "k")])).is_ok());
    for bad in [
        "PATH",
        "lower",
        "",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "A-B",
    ] {
        assert!(args_for(&with_extras(&[], &[(bad, "v")])).is_err(), "{bad}");
    }
}

#[test]
fn windows_search_paths() {
    let dirs = windows_dirs(
        Some("C:/L".into()),
        Some("C:/U".into()),
        Some("C:/A".into()),
    );
    assert_eq!(
        dirs,
        vec![
            PathBuf::from("C:/L/Programs/claude"),
            PathBuf::from("C:/U/.local/bin"),
            PathBuf::from("C:/A/npm"),
        ]
    );
    assert!(windows_dirs(None, None, None).is_empty());
}

#[test]
fn agy_error_stream_normalizes() {
    let got = events(DriverKind::Agy, include_str!("fixtures/agy.jsonl"));
    assert_eq!(got.len(), 1);
    assert!(matches!(&got[0], Event::Error { message } if message.starts_with("Eligibility check failed")));
}

#[test]
fn agy_success_result_normalizes() {
    let line = r#"{"event":"result","result":{"conversation_id":"c-1","status":"SUCCESS","response":"hi","error":""}}"#;
    assert_eq!(
        parse_line(DriverKind::Agy, line),
        vec![
            Event::Text { text: "hi".into() },
            Event::Done { session: Some("c-1".into()), ok: true, result: Some("hi".into()) },
        ]
    );
}

#[test]
fn agy_args_use_print_stream_json_and_conversation() {
    let args = args_for(&job(DriverKind::Agy, "-hi", Some("c-1"))).unwrap();
    assert_eq!(&args[..2], ["-p", "-hi"]);
    assert!(args.windows(2).any(|w| w == ["--output-format", "stream-json"]));
    assert!(args.windows(2).any(|w| w == ["--mode", "auto"]));
    assert!(args.windows(2).any(|w| w == ["--conversation", "c-1"]));
}

#[test]
fn hermes_plain_stdout_is_one_text_and_done() {
    let got = hermes_events(include_str!("fixtures/hermes.txt"));
    assert_eq!(got.len(), 2);
    assert!(matches!(&got[0], Event::Text { text } if !text.is_empty() && !text.ends_with('\n')));
    assert!(matches!(&got[1], Event::Done { ok: true, .. }));
    assert!(matches!(hermes_events("")[0], Event::Done { ok: false, .. }));
}

#[test]
fn hermes_args_attach_prompt_and_resume() {
    let args = args_for(&job(DriverKind::Hermes, "-x", Some("s_1"))).unwrap();
    assert_eq!(args, ["--resume", "s_1", "--oneshot=-x"]);
}

#[test]
fn fable_models_are_refused_for_every_driver() {
    for kind in DriverKind::ALL {
        let mut j = job(kind, "hi", None);
        j.env = vec![("ANTHROPIC_MODEL".into(), "claude-Fable-1".into())];
        assert!(matches!(args_for(&j), Err(DriverError::Invalid("model"))), "{kind:?}");
        let mut j = job(kind, "hi", None);
        j.extra_args = vec!["--model".into(), "fable".into()];
        assert!(matches!(args_for(&j), Err(DriverError::Invalid("model"))), "{kind:?}");
    }
}

#[test]
fn a_proxy_run_drops_inherited_anthropic_vars_and_the_telemetry_switch() {
    let inherited = || ["ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL", "PATH", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"].map(str::to_owned).into_iter();
    // A normal run inherits everything.
    assert!(inherited_to_remove(&with_extras(&[], &[]), inherited()).is_empty());
    // A proxy run keeps what it sets itself and drops the rest.
    let j = with_extras(&[], &[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8082")]);
    assert_eq!(
        inherited_to_remove(&j, inherited()),
        ["ANTHROPIC_API_KEY", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"]
    );
    // Only claude is a proxy client.
    let other = Job { driver: DriverKind::Codex, ..j };
    assert!(inherited_to_remove(&other, inherited()).is_empty());
}

#[test]
fn run_path_keeps_existing_dirs_once_in_order() {
    let tmp = std::env::temp_dir();
    let missing = tmp.join("kun-no-such-dir-xyz");
    let dirs = vec![tmp.clone(), missing, tmp.clone(), std::path::PathBuf::from("/")];
    let joined = super::run_path(&dirs);
    let parts: Vec<_> = std::env::split_paths(&joined).collect();
    assert_eq!(parts, vec![tmp, std::path::PathBuf::from("/")]);
}

fn claude_events(fixture: &str) -> Vec<Event> {
    events(DriverKind::Claude, fixture)
}

#[test]
fn unavailable_reasons_classify_from_real_cli_runs() {
    assert_eq!(
        classify_failure(&claude_events(include_str!("fixtures/claude-not-signed-in.jsonl"))),
        Some(Unavailable::NotSignedIn)
    );
    assert_eq!(
        classify_failure(&claude_events(include_str!("fixtures/claude-usage-limit.jsonl"))),
        Some(Unavailable::UsageLimit { resets_at: Some("9pm (Asia/Riyadh)".into()) })
    );
    assert_eq!(
        classify_failure(&claude_events(include_str!("fixtures/claude-usage-limit-session.jsonl"))),
        Some(Unavailable::UsageLimit { resets_at: Some("9pm (Asia/Riyadh)".into()) })
    );
    assert_eq!(
        classify_failure(&claude_events(include_str!("fixtures/claude-no-subscription.jsonl"))),
        Some(Unavailable::NoSubscription)
    );
}

#[test]
fn ordinary_failures_and_successes_are_not_unavailable() {
    assert_eq!(classify_failure(&claude_events(include_str!("fixtures/claude-error-other.jsonl"))), None);
    assert_eq!(classify_failure(&claude_events(include_str!("fixtures/claude.jsonl"))), None);
    // A successful answer that merely quotes a limit message is not a failure.
    let ok = [Event::Done { session: None, ok: true, result: Some("You've hit your limit".into()) }];
    assert_eq!(classify_failure(&ok), None);
    // Other drivers' errors, a bare non-zero exit, a long body.
    let other = events(DriverKind::Opencode, include_str!("fixtures/opencode-error.jsonl"));
    assert_eq!(classify_failure(&other), None);
    assert_eq!(classify_failure(&[Event::Done { session: None, ok: false, result: None }]), None);
    assert_eq!(classify_unavailable("rate limited, retry later"), None);
    assert_eq!(classify_unavailable(&format!("You've hit your {}", "x".repeat(500))), None);
}

#[test]
fn usage_limit_without_a_reset_time_has_none() {
    assert_eq!(
        classify_unavailable("Claude AI usage limit reached"),
        Some(Unavailable::UsageLimit { resets_at: None })
    );
}

#[test]
fn free_speaks_claude_stream_json_with_the_same_args() {
    assert_eq!(DriverKind::Free.binary(), "kun-free.sh");
    assert_eq!(DriverKind::Free.aliases(), ["f"]);
    assert!(DriverKind::ALL.contains(&DriverKind::Free));
    let free = args_for(&job(DriverKind::Free, "hi", Some("abc-1"))).unwrap();
    assert_eq!(free, args_for(&job(DriverKind::Claude, "hi", Some("abc-1"))).unwrap());
    assert!(free.windows(2).any(|w| w == ["--output-format", "stream-json"]));
    let got = events(DriverKind::Free, include_str!("fixtures/claude.jsonl"));
    assert_eq!(got, events(DriverKind::Claude, include_str!("fixtures/claude.jsonl")));
}

#[test]
fn free_is_found_under_its_alias() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("f");
    std::fs::write(&f, "#!/bin/sh\n").unwrap();
    let dirs = [dir.path().to_path_buf()];
    assert_eq!(find_driver(DriverKind::Free, &dirs), Some(f));
    assert_eq!(find_driver(DriverKind::Claude, &dirs), None);
}

#[test]
fn a_driver_spec_is_a_cli_name_or_a_web_seat() {
    assert_eq!(DriverKind::parse_spec("Claude"), Some((DriverKind::Claude, None)));
    assert_eq!(DriverKind::parse_spec(" web:chatgpt "), Some((DriverKind::Web, Some("chatgpt".into()))));
    assert_eq!(DriverKind::parse_spec("free"), Some((DriverKind::Free, None)));
    for bad in ["web", "web:", "web:../x", "web:Chat GPT", "gpt9", "kun-free.sh"] {
        assert_eq!(DriverKind::parse_spec(bad), None, "{bad}");
    }
}

#[test]
fn a_web_seat_is_not_a_program() {
    assert!(!DriverKind::Web.claude_like());
    assert!(!DriverKind::ALL.contains(&DriverKind::Web), "detect() never looks for a web binary");
    assert!(args_for(&job(DriverKind::Web, "hi", None)).is_err());
    assert!(parse_line(DriverKind::Web, "{\"type\":\"result\"}").is_empty());
}

#[tokio::test]
async fn a_web_job_needs_a_valid_seat_and_a_prompt() {
    let mut j = job(DriverKind::Web, "hi", None);
    assert!(matches!(run(&j, |_| {}).await, Err(DriverError::Invalid("web seat"))));
    j.seat = Some("../x".into());
    assert!(matches!(run(&j, |_| {}).await, Err(DriverError::Invalid("web seat"))));
    j.seat = Some("chatgpt".into());
    j.prompt = "  ".into();
    assert!(matches!(run(&j, |_| {}).await, Err(DriverError::Invalid("empty prompt"))));
    j.prompt = "hi".into();
    j.extra_args = vec!["--model".into(), "x".into()];
    assert!(run(&j, |_| {}).await.is_err(), "a web seat takes no CLI flags");
}
