use super::*;

fn set(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

#[test]
fn board_args_are_built_and_validated() {
    let a = board_args("video", Some("hogwarts"), "abdout", Some("red")).unwrap();
    assert_eq!(
        a,
        ["board", "--dept", "video", "--repo", "hogwarts", "--login", "abdout", "--health", "red", "--json"]
    );
    // No repo, no login, an unknown health level: only what is valid goes.
    let a = board_args("video", Some(""), "", Some("purple")).unwrap();
    assert_eq!(a, ["board", "--dept", "video", "--json"]);
    // A login that is not a login is dropped, a bad name is refused.
    assert!(!board_args("video", None, "A B", None).unwrap().contains(&"--login".to_owned()));
    assert!(matches!(board_args("../x", None, "", None), Err(PackError::BadName)));
    assert!(matches!(board_args("video", Some("a b"), "", None), Err(PackError::BadRepo(_))));
}

#[test]
fn fill_args_carry_each_slot_as_one_argument() {
    let a = fill_args("video", "video.film.plan", Some("mkan"), "sedon", &set(&[("market", "sd"), ("scene", "a b -- c")])).unwrap();
    assert_eq!(
        a,
        [
            "fill", "video.film.plan", "--dept", "video", "--repo", "mkan", "--login", "sedon",
            "--set", "market=sd", "--set", "scene=a b -- c", "--json"
        ]
    );
    assert!(fill_args("video", "../x", None, "", &set(&[])).is_err());
    assert!(fill_args("video", "VIDEO", None, "", &set(&[])).is_err());
    assert!(fill_args("video", "a.b", None, "", &set(&[("bad key", "x")])).is_err());
    assert!(fill_args("video", "a.b", None, "", &set(&[("k", "line\nbreak")])).is_err());
    assert!(fill_args("video", "a.b", None, "", &set(&[("k", &"x".repeat(301))])).is_err());
}

#[test]
fn typed_text_never_reads_as_a_flag() {
    assert_eq!(clean_text("  --dept sales  ").as_deref(), Some("dept sales"));
    assert_eq!(clean_text("-x").as_deref(), Some("x"));
    assert_eq!(clean_text("فيديو تعريفي").as_deref(), Some("فيديو تعريفي"));
    assert_eq!(clean_text("a\nb").as_deref(), Some("ab"));
    assert_eq!(clean_text(" - ").as_deref(), None);
    assert_eq!(clean_text(&"x".repeat(900)).unwrap().chars().count(), 500);
    assert_eq!(match_args("video", "  ", None).unwrap(), None);
    let a = match_args("video", "intro film", Some("hogwarts")).unwrap().unwrap();
    assert_eq!(a, ["match", "intro film", "--dept", "video", "--repo", "hogwarts", "--json"]);
}

const BOARD: &str = r#"{
  "department": "video", "repo": "hogwarts",
  "options": [
    {"role":"next","id":"video.film.continue","label_en":"Continue production","label_ar":"متابعة الإنتاج","icon":"Play","route":"do","record":"none","score":4.8,
     "why":["continue: open project","drive 0.9"],"set":{"project":"p-v001","n":3,"deep":{"x":1}}},
    {"role":"start","id":"video.film.plan","label_en":"New film","label_ar":"فيلم جديد","icon":"Film","route":"plan","record":"issue","score":2,"why":[],"set":{}},
    {"role":"start","id":"BAD ID","label_en":"x"},
    "not an object",
    {"role":"start","id":"video.script.only","label_en":"Video script","label_ar":"سكربت","route":"do","record":"none","score":1,"why":[],"set":{}},
    {"role":"start","id":"video.tools.check","label_en":"Tools","label_ar":"أدوات","record":"none","score":0.5,"why":[],"set":{}}
  ],
  "ms": 84
}"#;

#[test]
fn a_board_parses_in_order_with_at_most_three_options() {
    let b = parse_board(BOARD, "video", Some("hogwarts"));
    assert_eq!(b.error, None);
    assert_eq!(b.repo.as_deref(), Some("hogwarts"));
    let ids: Vec<_> = b.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["video.film.continue", "video.film.plan", "video.script.only"]);
    let first = &b.options[0];
    assert_eq!((first.role.as_str(), first.icon.as_deref(), first.route.as_deref()), ("next", Some("Play"), Some("do")));
    assert!((first.score - 4.8).abs() < f64::EPSILON);
    // Strings and numbers become facet values; an object is dropped.
    assert_eq!(first.set, set(&[("project", "p-v001"), ("n", "3")]));
    assert_eq!(b.options[1].record, "issue");
}

#[test]
fn a_board_that_is_not_a_board_is_empty_with_a_reason() {
    for bad in ["", "node: command not found", "[1,2]", r#"{"department":"video"}"#] {
        let b = parse_board(bad, "video", None);
        assert!(b.options.is_empty(), "{bad}");
    }
    assert!(parse_board("", "video", None).error.is_some());
    let b = parse_board(r#"{"error":"boom","department":"video","options":[]}"#, "video", None);
    assert_eq!((b.options.len(), b.error.as_deref()), (0, Some("boom")));
    // Noise before the JSON is tolerated.
    let b = parse_board(&format!("warning: x\n{BOARD}"), "video", None);
    assert_eq!(b.options.len(), 3);
}

#[test]
fn a_fill_keeps_unfilled_slots_and_lists_uploads() {
    let out = r###"{"id":"video.order.blender","prompt":"## Task\nBuild {{scene}} for {{to}}","route":"do","record":"issue",
        "missing":["scene"],"redirected_from":"video.film.plan",
        "uploads":[{"local":"/a/b.png","key":"work/f/ref/b.png","url":"https://cdn.databayt.org/work/f/ref/b.png"}]}"###;
    let f = parse_fill(out).unwrap();
    assert_eq!(f.id, "video.order.blender");
    assert!(f.prompt.contains("{{scene}}"));
    assert_eq!((f.route.as_deref(), f.record.as_str()), (Some("do"), "issue"));
    assert_eq!(f.missing, ["scene"]);
    assert_eq!(f.redirected_from.as_deref(), Some("video.film.plan"));
    assert_eq!(f.uploads[0].url, "https://cdn.databayt.org/work/f/ref/b.png");
    // The absent fields are tolerated.
    let f = parse_fill(r#"{"id":"a.b","prompt":"p"}"#).unwrap();
    assert!(f.missing.is_empty() && f.uploads.is_empty() && f.route.is_none());
}

#[test]
fn a_fill_carries_only_a_claude_model_and_a_known_effort() {
    let f = parse_fill(r#"{"id":"a.b","prompt":"p","model":"sonnet","effort":"medium"}"#).unwrap();
    assert_eq!((f.model.as_deref(), f.effort.as_deref()), (Some("sonnet"), Some("medium")));
    let f = parse_fill(r#"{"id":"a.b","prompt":"p","model":"fable","effort":"ludicrous"}"#).unwrap();
    assert_eq!((f.model, f.effort), (None, None));
}

#[test]
fn questions_args_carry_the_set_and_no_login() {
    let a = questions_args("video", "video.film.plan", Some("hogwarts"), &set(&[("market", "sa")])).unwrap();
    assert_eq!(
        a,
        ["questions", "video.film.plan", "--dept", "video", "--repo", "hogwarts", "--set", "market=sa", "--json"]
    );
    assert!(questions_args("video", "../x", None, &set(&[])).is_err());
    assert!(questions_args("video", "a.b", None, &set(&[("k", "a\nb")])).is_err());
}

#[test]
fn questions_parse_with_options_defaults_and_current_values() {
    let out = r#"{"id":"video.film.plan","questions":[
      {"slot":"market","kind":"market","q":{"en":"Which market?","ar":"أي سوق؟"},
       "options":[{"value":"sd","label":{"en":"Sudan · Arabic · 6.4 USD","ar":"السودان"}},{"value":"sa","label":{"en":"Saudi · Arabic · 24 SAR","ar":"السعودية"}}],
       "default":"sd","current":"sd"},
      {"slot":"length","kind":"choice","q":{"en":"How long?","ar":"كم المدة؟"},"options":[{"value":60,"label":{"en":"60 s","ar":"٦٠ ث"}}],"default":60,"current":null},
      {"slot":"note","kind":"ask","q":{"en":"Anything else?","ar":"شيء آخر؟"},"options":[],"default":null,"current":null},
      {"slot":"bad slot","kind":"ask"},
      "not an object"
    ]}"#;
    let q = parse_questions(out).unwrap();
    assert_eq!(q.id, "video.film.plan");
    assert_eq!(q.questions.len(), 3);
    assert_eq!(q.questions[0].options[1].value, "sa");
    assert_eq!(q.questions[0].current.as_deref(), Some("sd"));
    assert_eq!(q.questions[1].options[0].value, "60");
    assert_eq!(q.questions[1].default.as_deref(), Some("60"));
    assert_eq!((q.questions[2].kind.as_str(), q.questions[2].options.len()), ("ask", 0));
    // Nothing left to ask is an empty list, not an error.
    assert!(parse_questions(r#"{"id":"a.b","questions":[]}"#).unwrap().questions.is_empty());
    assert!(matches!(parse_questions(r#"{"error":"no such formula"}"#), Err(PackError::Station(m)) if m == "no such formula"));
    assert!(parse_questions("garbage").is_err());
}

#[test]
fn questions_carry_the_intro_and_option_sets() {
    let out = r#"{"id":"video.film.plan","intro":{"en":"Three quick questions.","ar":"ثلاثة أسئلة."},"questions":[
      {"slot":"cut","kind":"choice","q":{"en":"Cut?","ar":"نسخة؟"},"options":[
        {"value":"film","label":{"en":"2-minute film","ar":"فيلم"},"sets":{"platform":"Facebook","length":120,"skip":[1]}},
        {"value":"reel","label":{"en":"Reel","ar":"ريل"}}]}]}"#;
    let q = parse_questions(out).unwrap();
    assert_eq!(q.intro.as_ref().map(|w| w.en.as_str()), Some("Three quick questions."));
    let sets = q.questions[0].options[0].sets.as_ref().unwrap();
    assert_eq!((sets["platform"].as_str(), sets["length"].as_str(), sets.len()), ("Facebook", "120", 2));
    assert!(q.questions[0].options[1].sets.is_none());
    // A null or missing intro is none; a missing one never fails the parse.
    assert!(parse_questions(r#"{"id":"a.b","intro":null,"questions":[]}"#).unwrap().intro.is_none());
    assert!(parse_questions(r#"{"id":"a.b","questions":[]}"#).unwrap().intro.is_none());
}

#[test]
fn a_failed_fill_is_an_error_value() {
    assert!(matches!(parse_fill("garbage"), Err(PackError::Station(_))));
    assert!(matches!(parse_fill(r#"{"error":"no such formula"}"#), Err(PackError::Station(m)) if m == "no such formula"));
    assert!(parse_fill(r#"{"prompt":"x"}"#).is_err());
    assert!(parse_fill(r#"{"id":5}"#).is_err());
}

#[test]
fn a_match_parses_and_a_failure_is_no_match() {
    let m = parse_match(r#"{"id":"video.film.plan","confidence":0.92,"dept":null}"#);
    assert_eq!((m.id.as_deref(), m.dept), (Some("video.film.plan"), None));
    assert!((m.confidence - 0.92).abs() < f64::EPSILON);
    let m = parse_match(r#"{"id":null,"confidence":0,"dept":"sales"}"#);
    assert_eq!((m.id, m.dept.as_deref()), (None, Some("sales")));
    for bad in ["", "oops", r#"{"id":7}"#, r#"{"id":"../x","confidence":1}"#] {
        let m = parse_match(bad);
        assert_eq!(m.id, None, "{bad}");
    }
}

#[test]
fn the_script_must_live_under_kun_scripts() {
    let kun = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(kun.path().join(".claude/scripts")).unwrap();
    fs::create_dir_all(kun.path().join(".claude/skills/s")).unwrap();
    fs::write(kun.path().join(SCRIPT), "").unwrap();
    fs::write(kun.path().join(".claude/skills/s/x.mjs"), "").unwrap();
    let roots = [kun.path().join(SCRIPTS_DIR)];
    let script = kun.path().join(SCRIPT).display().to_string();
    let ok = health::safe_argv_under(&["node".into(), script, "board".into()], home.path(), &roots).unwrap();
    assert_eq!(ok[0], "node");
    assert_eq!(ok[2], "board");
    let other = kun.path().join(".claude/skills/s/x.mjs").display().to_string();
    assert!(health::safe_argv_under(&["node".into(), other], home.path(), &roots).is_err());
    assert!(health::safe_argv_under(&["sh".into(), "-c".into(), "id".into()], home.path(), &roots).is_err());
}

#[tokio::test]
async fn a_missing_script_is_an_error_not_a_crash() {
    let kun = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let r = run_station_in(kun.path(), home.path(), vec!["board".into()], Duration::from_secs(1)).await;
    assert!(r.is_err());
    let b = parse_board(&r.unwrap_err(), "video", None);
    assert!(b.options.is_empty());
}

#[test]
fn the_log_line_is_one_json_object() {
    let line = log_line("2026-11-22T09:30:00Z", "abdout", "video", Some("hogwarts"), "video.film.plan", LogEvent::Pick);
    assert_eq!(
        line,
        r#"{"ts":"2026-11-22T09:30:00Z","login":"abdout","dept":"video","repo":"hogwarts","id":"video.film.plan","event":"pick"}"#
    );
    let v: Value = serde_json::from_str(&log_line("t", "", "video", None, "a.b", LogEvent::Dismiss)).unwrap();
    assert_eq!((v["repo"].is_null(), v["event"].as_str()), (true, Some("dismiss")));
    assert!(!line.contains('\n'));
}

#[test]
fn times_are_written_in_utc() {
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(iso_utc(1_795_339_800), "2026-11-22T09:30:00Z");
}

#[test]
fn events_are_appended_one_line_each() {
    let machine = tempfile::tempdir().unwrap();
    log_event_in(machine.path(), "abdout", "video", Some("hogwarts"), "video.film.plan", LogEvent::Pick, 1_795_339_800).unwrap();
    log_event_in(machine.path(), "abdout", "video", None, "video.film.plan", LogEvent::Dismiss, 1_795_339_900).unwrap();
    let text = fs::read_to_string(log_path(machine.path())).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    let first: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["ts"], "2026-11-22T09:30:00Z");
    assert_eq!(first["event"], "pick");
    let second: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!((second["event"].as_str(), second["repo"].is_null()), (Some("dismiss"), true));
    // Bad input writes nothing.
    assert!(log_event_in(machine.path(), "", "../x", None, "a.b", LogEvent::Pick, 0).is_err());
    assert!(log_event_in(machine.path(), "", "video", Some("a b"), "a.b", LogEvent::Pick, 0).is_err());
    assert!(log_event_in(machine.path(), "", "video", None, "BAD", LogEvent::Pick, 0).is_err());
    assert_eq!(fs::read_to_string(log_path(machine.path())).unwrap().lines().count(), 2);
}

#[tokio::test]
async fn the_real_station_answers_board_fill_and_match() {
    let Ok(kun) = kun_dir() else { return };
    let Some(home) = dirs::home_dir() else { return };
    if !kun.join(SCRIPT).is_file() {
        eprintln!("skipped: no station script at {}", kun.display());
        return;
    }
    let run = |args: Vec<String>, t: Duration| run_station_in(&kun, &home, args, t);
    let out = match run(board_args("video", Some("hogwarts"), "", None).unwrap(), BOARD_TIMEOUT).await {
        Ok(o) => o,
        Err(e) if e.contains("node was not found") => return,
        Err(e) => panic!("{e}"),
    };
    let board = parse_board(&out, "video", Some("hogwarts"));
    assert!(board.error.is_none(), "{:?}", board.error);
    assert!(!board.options.is_empty() && board.options.len() <= MAX_OPTIONS);
    let first = &board.options[0];
    // A tap: fill the option with its own slots.
    let args = fill_args("video", &first.id, Some("hogwarts"), "", &first.set).unwrap();
    let filled = parse_fill(&run(args, FILL_TIMEOUT).await.unwrap()).unwrap();
    assert!(filled.prompt.contains("## Task"), "{}", filled.prompt);
    // Typed text: the switch.
    let args = match_args("video", "plan an intro film", Some("hogwarts")).unwrap().unwrap();
    let m = parse_match(&run(args, MATCH_TIMEOUT).await.unwrap());
    assert!(m.confidence.is_finite());
}

#[test]
fn run_line_carries_the_clock_as_iso_times() {
    let c = RunClock {
        tap: Some(1_763_800_000_000),
        send: Some(1_763_800_002_500),
        ask: Some(1_763_800_002_520),
        setup: Some(1_763_800_002_700),
        spawn: Some(1_763_800_002_710),
        first_event: Some(1_763_800_006_000),
        done: Some(1_763_800_031_999),
    };
    let line = run_line("abdout", "video", Some("hogwarts"), Some("video.film.plan"), true, &c);
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["event"], "run");
    assert_eq!(v["id"], "video.film.plan");
    assert_eq!(v["t_tap"], "2025-11-22T08:26:40.000Z");
    assert_eq!(v["t_send"], "2025-11-22T08:26:42.500Z");
    assert_eq!(v["t_done"], "2025-11-22T08:27:11.999Z");
    assert_eq!(v["ts"], v["t_done"]);
    assert_eq!(v["ok"], true);
    // A run typed by hand has no tap; the point is null, never made up.
    let hand = run_line("", "video", None, None, false, &RunClock { send: Some(1), done: Some(2), ..RunClock::default() });
    let h: serde_json::Value = serde_json::from_str(&hand).unwrap();
    assert!(h["t_tap"].is_null() && h["id"].is_null() && h["repo"].is_null());
    assert_eq!(h["ok"], false);
}

#[test]
fn iso_with_milliseconds_pads_them() {
    assert_eq!(iso_utc_ms(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso_utc_ms(1_005), "1970-01-01T00:00:01.005Z");
}
