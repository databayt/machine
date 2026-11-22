use std::path::Path;

use serde_json::json;

use super::{
    donor::{self, Snapshot, Tools, Why},
    model::{PoolFile, PoolJob, PoolNeeds},
    params::{self, Engine, Params, Preset, RenderFormat, TextFormat},
    *,
};
use crate::settings::ShareSettings;

fn job(kind: &str, params: serde_json::Value) -> PoolJob {
    PoolJob {
        id: "j1".into(),
        kind: kind.into(),
        params,
        requester: "ali".into(),
        target: None,
        donor: None,
        status: "queued".into(),
        inputs: vec![PoolFile { name: "a.mp4".into(), key: "k".into(), size: 1_000_000, content_type: "video/mp4".into(), get_url: None }],
        outputs: None,
        needs: None,
        progress: 0.0,
        log: String::new(),
        claims: 0,
        created_at: "t".into(),
        updated_at: "t".into(),
        finished_at: None,
    }
}

fn snap() -> Snapshot {
    Snapshot {
        share: ShareSettings { enabled: true, kinds: vec!["transcribe".into(), "render".into(), "encode".into()], idle_minutes: 10 },
        on_ac: Some(true),
        idle_secs: Some(900.0),
        ram_free: Some(16e9),
        disk_free: Some(200e9),
        has_gpu: Some(true),
        tools: Tools { whisper: true, ffmpeg: true, blender: true },
        models: vec!["ggml-base.bin".into()],
        doctor_red: false,
    }
}

// ---- eligibility ---------------------------------------------------------------------------------

#[test]
fn a_red_doctor_makes_the_machine_not_eligible_with_the_doctor_reason() {
    let mut s = snap();
    assert_eq!(donor::gate(&s), Ok(()));
    s.doctor_red = true;
    assert_eq!(donor::gate(&s), Err(Why::Doctor));
    assert_eq!(Why::Doctor.code(), "doctor");
}

#[test]
fn the_gate_wants_opt_in_kinds_mains_and_idle_time() {
    assert_eq!(donor::gate(&snap()), Ok(()));
    let mut s = snap();
    s.share.enabled = false;
    assert_eq!(donor::gate(&s), Err(Why::Off));
    let mut s = snap();
    s.share.kinds.clear();
    assert_eq!(donor::gate(&s), Err(Why::NoKinds));
    let mut s = snap();
    s.on_ac = Some(false);
    assert_eq!(donor::gate(&s), Err(Why::Battery));
    let mut s = snap();
    s.idle_secs = Some(599.0);
    assert_eq!(donor::gate(&s), Err(Why::Active));
    s.idle_secs = Some(600.0);
    assert_eq!(donor::gate(&s), Ok(()));
}

#[test]
fn an_unreadable_power_or_idle_reading_is_never_eligible() {
    let mut s = snap();
    s.on_ac = None;
    assert_eq!(donor::gate(&s), Err(Why::Unknown));
    let mut s = snap();
    s.idle_secs = None;
    assert_eq!(donor::gate(&s), Err(Why::Unknown));
}

#[test]
fn a_job_must_fit_the_kind_the_tool_the_model_and_the_needs() {
    let encode = job("encode", json!({ "preset": "proxy" }));
    assert_eq!(donor::job_fits(&encode, &snap()), Ok(()));
    // kind not ticked
    let mut s = snap();
    s.share.kinds = vec!["render".into()];
    assert!(donor::job_fits(&encode, &s).is_err());
    // ticked but the tool is not installed
    let mut s = snap();
    s.tools.ffmpeg = false;
    assert!(donor::job_fits(&encode, &s).is_err());
    assert!(!donor::kinds_available(&s.tools).contains(&"transcribe".to_owned()), "transcribe needs ffmpeg too");
    // bad params are refused locally, as kun's schema would
    assert!(donor::job_fits(&job("encode", json!({ "preset": "x264; echo hi" })), &snap()).is_err());
    // a transcribe job needs the model it names
    let t = job("transcribe", json!({ "format": "srt", "model": "large-v3" }));
    assert!(donor::job_fits(&t, &snap()).is_err());
    assert_eq!(donor::job_fits(&job("transcribe", json!({ "format": "srt", "model": "base" })), &snap()), Ok(()));
    assert_eq!(donor::job_fits(&job("transcribe", json!({ "format": "txt" })), &snap()), Ok(()));
    // memory, disk and GPU
    let mut big = job("render", json!({ "engine": "cycles", "frameStart": 1, "frameEnd": 10, "format": "png" }));
    big.needs = Some(PoolNeeds { ram_bytes: Some(32e9), gpu: None, disk_bytes: None });
    assert!(donor::job_fits(&big, &snap()).is_err());
    big.needs = Some(PoolNeeds { ram_bytes: None, gpu: Some(true), disk_bytes: Some(500e9) });
    assert!(donor::job_fits(&big, &snap()).is_err(), "disk");
    big.needs = Some(PoolNeeds { ram_bytes: None, gpu: Some(true), disk_bytes: None });
    let mut s = snap();
    s.has_gpu = Some(false);
    assert!(donor::job_fits(&big, &s).is_err(), "gpu");
    assert_eq!(donor::job_fits(&big, &snap()), Ok(()));
    // a running job is not claimable
    let mut taken = encode;
    taken.status = "running".into();
    assert!(donor::job_fits(&taken, &snap()).is_err());
}

// ---- templates: validation and argument vectors --------------------------------------------------

#[test]
fn parameters_are_validated_like_kuns_schemas() {
    assert_eq!(params::validate("encode", &json!({ "preset": "hevc" })), Ok(Params::Encode { preset: Preset::Hevc }));
    assert!(params::validate("encode", &json!({})).is_err());
    assert!(params::validate("claude", &json!({})).is_err(), "unknown kinds, such as a Claude run, never validate");
    assert!(params::validate("render", &json!({ "engine": "cycles", "frameStart": 10, "frameEnd": 5, "format": "png" })).is_err());
    assert!(params::validate("render", &json!({ "engine": "cycles", "frameStart": 0, "frameEnd": 1_000_001, "format": "png" })).is_err());
    assert_eq!(
        params::validate("render", &json!({ "engine": "eevee", "frameStart": 1, "frameEnd": 24, "format": "mp4" })),
        Ok(Params::Render { engine: Engine::Eevee, start: 1, end: 24, format: RenderFormat::Mp4 })
    );
    assert!(params::validate("transcribe", &json!({ "format": "srt", "language": "ARA" })).is_err());
    assert!(params::validate("transcribe", &json!({ "format": "srt", "language": "ar; ls" })).is_err());
    assert_eq!(
        params::validate("transcribe", &json!({ "format": "txt", "language": "ar", "model": "small" })),
        Ok(Params::Transcribe { model: Some("small".into()), language: Some("ar".into()), format: TextFormat::Txt })
    );
}

#[test]
fn a_hostile_name_stays_one_argument_and_never_reaches_a_shell() {
    let evil = Path::new("/tmp/x; echo hi $(touch pwned).mp4");
    let c = params::encode_cmd(Path::new("/usr/bin/ffmpeg"), Preset::H264Web, false, evil, Path::new("/tmp/out.mp4"));
    assert_eq!(c.program, "/usr/bin/ffmpeg");
    assert!(c.args.contains(&evil.display().to_string()), "the path is one argument, untouched");
    assert!(c.args.windows(2).any(|w| w == ["-crf", "23"]) && c.args.contains(&"+faststart".to_owned()));
    assert!(!params::safe_name("../../etc/passwd").contains('/'));
    assert!(!params::safe_name(".hidden").starts_with('.'));
    assert_eq!(params::safe_name(""), "file");
}

#[test]
fn the_presets_are_the_fixed_ffmpeg_lines() {
    let f = Path::new("ffmpeg");
    let (i, o) = (Path::new("in.mov"), Path::new("out.mp4"));
    let proxy = params::encode_cmd(f, Preset::Proxy, false, i, o);
    assert!(proxy.args.contains(&"scale=-2:720".to_owned()) && proxy.args.contains(&"ultrafast".to_owned()));
    let hevc_mac = params::encode_cmd(f, Preset::Hevc, true, i, o);
    assert!(hevc_mac.args.contains(&"hevc_videotoolbox".to_owned()));
    let hevc = params::encode_cmd(f, Preset::Hevc, false, i, o);
    assert!(hevc.args.contains(&"libx265".to_owned()) && hevc.args.windows(2).any(|w| w == ["-crf", "28"]));
    assert_eq!(hevc.args.last().map(String::as_str), Some("out.mp4"));
}

#[test]
fn whisper_blender_and_tar_lines() {
    let w = params::whisper_cmd(Path::new("whisper-cli"), Path::new("/m/ggml-base.bin"), Path::new("/w/0.wav"), Path::new("/o/a"), TextFormat::Srt, None);
    assert!(w.args.contains(&"-osrt".to_owned()) && w.args.windows(2).any(|a| a == ["-l", "auto"]));
    let w = params::whisper_cmd(Path::new("whisper-cli"), Path::new("/m/m.bin"), Path::new("/w/0.wav"), Path::new("/o/a"), TextFormat::Txt, Some("ar"));
    assert!(w.args.contains(&"-otxt".to_owned()) && w.args.windows(2).any(|a| a == ["-l", "ar"]));
    let b = params::blender_render_cmd(Path::new("blender"), "CYCLES", Path::new("/in/s.blend"), Path::new("/o"), 1, 48);
    assert!(b.args.contains(&"--disable-autoexec".to_owned()), "a .blend is a program");
    assert_eq!(b.args.last().map(String::as_str), Some("-a"), "-a comes after every setting");
    assert!(b.args.windows(2).any(|a| a == ["-s", "1"]) && b.args.windows(2).any(|a| a == ["-e", "48"]));
    assert_eq!(params::engine_id(Engine::Eevee, (5, 3)), "BLENDER_EEVEE");
    assert_eq!(params::engine_id(Engine::Eevee, (4, 3)), "BLENDER_EEVEE_NEXT");
    assert_eq!(params::engine_id(Engine::Eevee, (4, 1)), "BLENDER_EEVEE");
    assert_eq!(params::engine_id(Engine::Cycles, (5, 3)), "CYCLES");
    assert_eq!(params::parse_blender_version("Blender 5.3.0\n\tbuild date: x"), Some((5, 3)));
    let t = params::tar_cmd(Path::new("/o/frames.tar.gz"), Path::new("/w/frames"));
    assert_eq!(t.program, "tar");
    assert_eq!(params::parse_kun_fps("Blender 5\nKUNFPS 30\n"), Some(30));
}

#[test]
fn progress_lines_parse() {
    assert_eq!(params::ffmpeg_out_time("out_time_us=2500000"), Some(2.5));
    assert_eq!(params::ffmpeg_out_time("frame=12"), None);
    assert_eq!(params::blender_frame("Fra:12 Mem:34.5M (Peak 40M) | Time:00:01.2"), Some(12));
    assert_eq!(params::blender_frame("Saved: 'x.png'"), None);
    assert_eq!(params::whisper_progress("whisper_print_progress_callback: progress =  45%"), Some(0.45));
    assert_eq!(params::parse_duration("12.5\n"), Some(12.5));
}

#[test]
fn priority_is_lowered_with_a_fixed_prefix() {
    let c = params::Cmd { program: "ffmpeg".into(), args: vec!["-i".into(), "x".into()] };
    let l = donor::lowered(&c);
    if cfg!(unix) {
        assert_eq!(l.program, "nice");
        assert_eq!(&l.args[..3], ["-n", "10", "ffmpeg"]);
        assert_eq!(&l.args[3..], ["-i", "x"]);
    } else {
        assert_eq!(l, c);
    }
}

#[test]
fn local_names_are_safe_and_distinct() {
    let f = |n: &str| PoolFile { name: n.into(), key: "k".into(), size: 1, content_type: "x".into(), get_url: None };
    let names = donor::local_names(&[f("a.mp4"), f("a.mp4"), f("../b.mov")]);
    assert_eq!(names[0], "a.mp4");
    assert_eq!(names[1], "1-a.mp4");
    assert!(!names[2].contains('/'));
    assert!(donor::valid_id("cmabc123_-x") && !donor::valid_id("../x") && !donor::valid_id(""));
    assert_eq!(donor::output_type("frames.tar.gz"), "application/gzip");
}

#[test]
fn models_are_found_by_size_and_name_and_picked_by_param() {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![0u8; 10_000_001];
    std::fs::write(dir.path().join("ggml-base.bin"), &big).unwrap();
    std::fs::write(dir.path().join("ggml-small.bin"), &big).unwrap();
    std::fs::write(dir.path().join("ggml-tiny.test.bin"), &big).unwrap();
    std::fs::write(dir.path().join("ggml-stub.bin"), b"tiny").unwrap();
    let found = donor::find_models(&[dir.path().to_path_buf()]);
    assert_eq!(found.len(), 2, "no stubs, no test files: {found:?}");
    assert!(donor::pick_model(&found, Some("small")).unwrap().ends_with("ggml-small.bin"));
    assert!(donor::pick_model(&found, Some("medium")).is_none());
    assert!(donor::pick_model(&found, None).is_some());
}

// ---- running a child, the protocol ---------------------------------------------------------------

#[cfg(unix)]
#[tokio::test]
async fn a_child_runs_reports_its_lines_and_is_killed_on_cancel() {
    let ctl = donor::Ctl::default();
    let mut seen = Vec::new();
    let ok = params::Cmd { program: "sh".into(), args: vec!["-c".into(), "echo one; echo two 1>&2".into()] };
    donor::exec(&ok, &ctl, |l| seen.push(l.to_owned())).await.unwrap();
    seen.sort();
    assert_eq!(seen, ["one", "two"]);
    let bad = params::Cmd { program: "sh".into(), args: vec!["-c".into(), "exit 3".into()] };
    assert!(matches!(donor::exec(&bad, &ctl, |_| {}).await, Err(donor::RunError::Failed(_))));
    let slow = params::Cmd { program: "sleep".into(), args: vec!["30".into()] };
    let ctl = Arc::new(donor::Ctl::default());
    let c2 = ctl.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        c2.abort(donor::Abort::Active);
    });
    let started = std::time::Instant::now();
    let r = donor::exec(&slow, &ctl, |_| {}).await;
    assert_eq!(r, Err(donor::RunError::Aborted(donor::Abort::Active)));
    assert!(started.elapsed() < Duration::from_secs(5), "the child was killed, not waited for");
}

type Answers = Vec<(&'static str, Result<&'static str, (u16, &'static str)>)>;

struct Mock {
    answers: Answers,
    seen: Mutex<Vec<(String, String)>>,
}

impl Api for Mock {
    async fn get(&self, path: &str) -> Result<String, ApiError> {
        self.reply(path, "")
    }
    async fn post(&self, path: &str, body: String) -> Result<String, ApiError> {
        self.reply(path, &body)
    }
}

impl Mock {
    fn new(answers: Answers) -> Self {
        Self { answers, seen: Mutex::default() }
    }
    fn reply(&self, path: &str, body: &str) -> Result<String, ApiError> {
        self.seen.lock().unwrap().push((path.to_owned(), body.to_owned()));
        let (_, r) = self.answers.iter().find(|(p, _)| *p == path).expect("an unexpected call");
        match r {
            Ok(s) => Ok((*s).to_owned()),
            Err((status, b)) => Err(ApiError::Status { status: *status, body: (*b).to_owned() }),
        }
    }
}

#[tokio::test]
async fn claim_reads_kuns_refusals_including_doctor() {
    let m = Mock::new(vec![("/api/machine/pool/claim", Err((409, r#"{"ok":false,"error":"doctor"}"#)))]);
    assert_eq!(donor::claim(&m, "j1").await.unwrap_err(), Refused::Reason("doctor".into()));
    let m = Mock::new(vec![("/api/machine/pool/claim", Err((409, r#"{"ok":false,"error":"taken"}"#)))]);
    assert_eq!(donor::claim(&m, "j1").await.unwrap_err(), Refused::Reason("taken".into()));
    let m = Mock::new(vec![("/api/machine/pool/claim", Err((500, "x")))]);
    assert!(matches!(donor::claim(&m, "j1").await, Err(Refused::Api(_))));
}

#[tokio::test]
async fn progress_says_when_the_job_is_no_longer_ours() {
    let m = Mock::new(vec![("/api/machine/pool/progress", Ok(r#"{"ok":false,"cancelled":true}"#))]);
    assert!(donor::report(&m, "j1", 0.5, "log").await);
    let body: serde_json::Value = serde_json::from_str(&m.seen.lock().unwrap()[0].1).unwrap();
    assert_eq!(body["progress"], 0.5);
    let m = Mock::new(vec![("/api/machine/pool/progress", Ok(r#"{"ok":true,"cancelled":false}"#))]);
    assert!(!donor::report(&m, "j1", 0.5, "").await);
}

#[tokio::test]
async fn releasing_posts_cancel_for_the_job() {
    let m = Mock::new(vec![("/api/machine/pool/cancel", Ok(r#"{"ok":true,"released":true}"#))]);
    donor::release(&m, "j9").await;
    assert_eq!(m.seen.lock().unwrap()[0].0, "/api/machine/pool/cancel");
}

#[test]
fn needs_follow_the_kind_and_the_input_size() {
    let n = needs_for("render", 1000);
    assert_eq!(n.disk_bytes, Some(3000.0));
    assert!(n.ram_bytes.unwrap() > needs_for("encode", 1).ram_bytes.unwrap());
}

#[test]
fn the_donor_status_wire_is_camel_case_and_a_job_hides_its_links() {
    let st = model::DonorStatus { state: "waiting".into(), reason: Some("doctor".into()), kinds_available: vec!["encode".into()], ..Default::default() };
    let v = serde_json::to_value(&st).unwrap();
    assert!(v.get("kindsAvailable").is_some() && v.get("lastError").is_some());
    let f: PoolFile = serde_json::from_str(r#"{"name":"a","key":"k","size":1,"contentType":"x","getUrl":"https://s3/x"}"#).unwrap();
    assert_eq!(f.get_url.as_deref(), Some("https://s3/x"));
    assert!(serde_json::to_value(&f).unwrap().get("getUrl").is_none(), "presigned links never reach the page");
}
