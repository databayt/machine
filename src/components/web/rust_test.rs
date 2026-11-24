use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::*;

const INDEX: &str = r#"[
  {"id":"chatgpt","name":"ChatGPT","url":"https://chatgpt.com/"},
  {"id":"deepseek","name":"DeepSeek","url":"https://chat.deepseek.com/"},
  {"id":"Bad Id","name":"x","url":"https://x.test/"},
  {"id":"plain","name":"Plain","url":"http://insecure.test/"},
  {"id":"glm","name":" ","url":"https://chat.z.ai/"}
]"#;

#[test]
fn the_index_keeps_only_safe_rows() {
    let seats = parse_index(INDEX).unwrap();
    let ids: Vec<&str> = seats.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["chatgpt", "deepseek"], "bad id, http url and blank name are dropped");
    assert!(matches!(parse_index("{}"), Err(WebError::NoIndex)));
    assert!(matches!(parse_index("nope"), Err(WebError::NoIndex)));
}

#[test]
fn seat_ids_are_short_lowercase_words() {
    assert!(valid_seat_id("chatgpt") && valid_seat_id("glm5"));
    for bad in ["", "Chat", "a-b", "../x", "a b", "a/b", &"x".repeat(25)] {
        assert!(!valid_seat_id(bad), "{bad:?}");
    }
}

#[test]
fn the_index_and_adapter_load_from_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(load_index(dir.path()), Err(WebError::NoIndex)));
    std::fs::write(dir.path().join("index.json"), INDEX).unwrap();
    std::fs::write(dir.path().join("chatgpt.js"), "window.__kunWeb = {};").unwrap();
    assert_eq!(find_seat(dir.path(), "chatgpt").unwrap().name, "ChatGPT");
    assert!(matches!(find_seat(dir.path(), "gemini"), Err(WebError::UnknownSeat(_))));
    assert!(matches!(find_seat(dir.path(), "../index"), Err(WebError::UnknownSeat(_))));
    assert_eq!(read_adapter(dir.path(), "chatgpt").as_deref(), Some("window.__kunWeb = {};"));
    assert_eq!(read_adapter(dir.path(), "deepseek"), None, "no adapter file yet is not an error");
}

#[test]
fn scripts_carry_the_prompt_as_one_json_string() {
    let nasty = "line1\n\"quoted\" </script> \\ end";
    let js = send_script("window.__kunWeb = { id: 'x' };", nasty);
    assert!(js.contains(&serde_json::to_string(nasty).unwrap()), "the prompt is a JSON literal: {js}");
    assert!(js.contains("window.__kunWeb = { id: 'x' };"), "the adapter is re-injected when lost");
    assert!(js.contains("w.send("));
    assert!(js.contains("JSON.stringify"));
    let bare = status_script("");
    assert!(!bare.contains("try {\n\n}"), "no adapter text, nothing to inject");
    assert!(bare.contains("needsLogin") && bare.contains("limitHit") && bare.contains("lastAnswer"));
    assert!(transcript_script("").contains("w.transcript()"));
    assert!(init_script("var a = 1;").starts_with("try {"));
}

#[test]
fn a_page_answer_decodes_whatever_wry_hands_back() {
    // wry returns the JSON of the value: a stringified object is a JSON string of JSON.
    let obj = r#"{"adapter":true,"busy":false}"#;
    let wrapped = serde_json::to_string(obj).unwrap();
    assert_eq!(decode_result(&wrapped).unwrap()["adapter"], true);
    assert_eq!(decode_result(obj).unwrap()["busy"], false, "an object passes through");
    assert_eq!(decode_result("").unwrap(), Value::Null, "an exception gives an empty answer");
    assert_eq!(decode_result("null").unwrap(), Value::Null);
    assert!(matches!(decode_result("\"not json\""), Err(WebError::Page(_))));
}

#[test]
fn a_status_reads_the_adapters_four_answers() {
    let v: Value = serde_json::from_str(r#"{"adapter":true,"ready":true,"needsLogin":false,"limit":" You hit the cap ","busy":true,"url":"https://chatgpt.com/c/1"}"#).unwrap();
    let st = status_from(&v);
    assert!(st.adapter && st.ready && st.busy && !st.needs_login);
    assert_eq!(st.limit.as_deref(), Some(" You hit the cap "));
    let none = status_from(&serde_json::from_str(r#"{"adapter":false,"limit":null}"#).unwrap());
    assert_eq!(none, SeatStatus::default());
}

#[test]
fn the_min_gap_is_counted_from_the_last_start() {
    let now = Instant::now();
    assert_eq!(gap_wait(None, now, MIN_GAP), Duration::ZERO);
    let then = now.checked_sub(Duration::from_secs(10)).unwrap();
    assert_eq!(gap_wait(Some(then), now, MIN_GAP), Duration::from_secs(35));
    let long_ago = now.checked_sub(Duration::from_secs(90)).unwrap();
    assert_eq!(gap_wait(Some(long_ago), now, MIN_GAP), Duration::ZERO);
}

#[test]
fn an_answer_is_final_when_the_seat_is_idle_and_the_text_holds_still_for_three_seconds() {
    let s = Duration::from_secs;
    let mut settle = Settle::new("old answer");
    // The old answer is still on the page and the seat is idle: nothing yet.
    assert_eq!(settle.feed(s(1), false, "old answer"), None);
    // Working: partial text must not count.
    assert_eq!(settle.feed(s(2), true, "Par"), None);
    assert_eq!(settle.feed(s(3), true, "Partial answ"), None);
    // Idle with the final text: the clock starts, 3 s must pass unchanged.
    assert_eq!(settle.feed(s(4), false, "Partial answer."), None);
    assert_eq!(settle.feed(s(5), false, "Partial answer."), None);
    assert_eq!(settle.feed(s(6), false, "Partial answer."), None);
    assert_eq!(settle.feed(s(7), false, "Partial answer.").as_deref(), Some("Partial answer."));
}

#[test]
fn a_text_that_changes_or_a_seat_that_works_again_restarts_the_clock() {
    let s = Duration::from_secs;
    let mut settle = Settle::new("");
    assert_eq!(settle.feed(s(1), false, "A"), None);
    assert_eq!(settle.feed(s(3), false, "AB"), None, "changed: restart");
    assert_eq!(settle.feed(s(5), true, "AB"), None, "busy again: restart");
    assert_eq!(settle.feed(s(6), false, "AB"), None);
    assert_eq!(settle.feed(s(9), false, "AB").as_deref(), Some("AB"));
}

#[test]
fn the_same_answer_counts_when_the_seat_was_seen_working() {
    let s = Duration::from_secs;
    let mut settle = Settle::new("Yes.");
    assert_eq!(settle.feed(s(1), true, "Yes."), None);
    assert_eq!(settle.feed(s(2), false, "Yes."), None);
    assert_eq!(settle.feed(s(5), false, "Yes.").as_deref(), Some("Yes."));
}

#[test]
fn a_seat_takes_one_ask_at_a_time_and_frees_on_drop() {
    let id = "gate1";
    let now = Instant::now();
    let first = SeatGuard::take(id, now).unwrap();
    assert!(first.wait.is_zero(), "never asked before");
    assert!(matches!(SeatGuard::take(id, now), Err(WebError::Busy(_))));
    state().seats.entry(id.to_owned()).or_default().last_start = Some(now);
    drop(first);
    let second = SeatGuard::take(id, now + Duration::from_secs(5)).unwrap();
    assert_eq!(second.wait, Duration::from_secs(40), "the 45 s gap runs from the last start");
}

#[test]
fn a_seats_store_id_is_stable_and_per_seat() {
    assert_eq!(store_id("chatgpt"), store_id("chatgpt"));
    assert_ne!(store_id("chatgpt"), store_id("gemini"));
}

#[test]
fn a_seat_may_only_load_https_pages() {
    let ok = |u: &str| allowed_navigation(&u.parse().unwrap());
    assert!(ok("https://chatgpt.com/c/1") && ok("about:blank"));
    for bad in ["http://chatgpt.com/", "tauri://localhost/en/", "file:///etc/passwd", "data:text/html,hi", "javascript:alert(1)", "ipc://localhost/x"] {
        assert!(!ok(bad), "{bad}");
    }
}

// ---- the ask loop against a scripted page

/// A page that answers `status` scripts from a script of (job state, busy, answer) steps.
struct FakePage {
    exists: Mutex<bool>,
    steps: Mutex<Vec<Value>>,
    sent: Mutex<Vec<String>>,
}

impl FakePage {
    fn new(steps: Vec<Value>) -> Arc<Self> {
        Arc::new(Self { exists: Mutex::new(true), steps: Mutex::new(steps), sent: Mutex::new(Vec::new()) })
    }
}

impl Backend for FakePage {
    fn seat_exists(&self, _id: &str) -> bool {
        *self.exists.lock().unwrap()
    }
    fn create_seat(&self, _seat: &Seat, _init: &str, _rect: Rect, _visible: bool) -> Result<(), WebError> {
        *self.exists.lock().unwrap() = true;
        Ok(())
    }
    fn place_seat(&self, _id: &str, _rect: Rect, _focus: bool) -> Result<(), WebError> {
        Ok(())
    }
    fn hide_seat(&self, _id: &str) -> Result<(), WebError> {
        Ok(())
    }
    fn hide_all_seats(&self) {}
    fn close_seat(&self, _id: &str) {}
    fn eval_seat(&self, _id: &str, js: String, done: Box<dyn Fn(String) + Send>) -> Result<(), WebError> {
        let answer = if js.contains("started: true") {
            self.sent.lock().unwrap().push(js);
            serde_json::json!({"adapter": true, "started": true})
        } else {
            let mut steps = self.steps.lock().unwrap();
            if steps.len() > 1 { steps.remove(0) } else { steps[0].clone() }
        };
        done(serde_json::to_string(&answer.to_string()).unwrap());
        Ok(())
    }
}

fn st(job: &str, busy: bool, answer: &str) -> Value {
    serde_json::json!({"adapter": true, "ready": true, "needsLogin": false, "limit": null, "busy": busy, "answer": answer, "job": {"state": job}})
}

#[test]
fn eval_value_returns_what_the_page_calls_back() {
    let page = FakePage::new(vec![st("sent", false, "hi")]);
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let v = rt.block_on(eval_value(page.as_ref(), "x", "status".into())).unwrap();
    assert_eq!(v["answer"], "hi");
}

#[test]
fn a_limit_message_ends_the_wait_with_limit_text() {
    let mut v = st("run", false, "");
    v["limit"] = Value::String("Daily limit reached".into());
    let page = FakePage::new(vec![v]);
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let got = rt.block_on(wait_ready(page.as_ref(), "x", "", "ChatGPT"));
    match got {
        Err(WebError::Page(m)) => assert_eq!(m, "limit: Daily limit reached"),
        other => panic!("expected the limit error, got {other:?}"),
    }
}

#[test]
fn a_ready_seat_passes_the_ready_wait() {
    let page = FakePage::new(vec![st("run", false, "")]);
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    assert!(rt.block_on(wait_ready(page.as_ref(), "x", "", "ChatGPT")).is_ok());
}

#[tokio::test(start_paused = true)]
async fn a_signed_out_seat_ends_with_login_text() {
    let mut v = st("run", false, "");
    v["ready"] = Value::Bool(false);
    v["needsLogin"] = Value::Bool(true);
    let page = FakePage::new(vec![v]);
    match wait_ready(page.as_ref(), "x", "", "DeepSeek").await {
        Err(WebError::Page(m)) => assert!(m.starts_with("login: DeepSeek"), "{m}"),
        other => panic!("expected the login error, got {other:?}"),
    }
}

#[test]
fn without_a_backend_the_driver_door_says_so() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let mut got = Vec::new();
    rt.block_on(run_seat("chatgpt", "hi", |e| got.push(e)));
    // No app is running in a unit test: one error event, no panic.
    assert_eq!(got, vec![Event::Error { message: WebError::NoBackend.to_string() }]);
}
