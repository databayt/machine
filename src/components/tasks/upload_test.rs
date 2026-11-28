use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;
use crate::network::api::ApiError;

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// One-shot HTTP server for a presigned PUT: answers `status`, returns the head and the body it got.
async fn serve_put(status: u16) -> (String, tokio::task::JoinHandle<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/work/f/1/clip.mp4?sig=abc", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let head_end = loop {
            let n = sock.read(&mut chunk).await.unwrap();
            assert!(n > 0, "closed before the head ended");
            buf.extend_from_slice(&chunk[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let len: usize = head
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
            .expect("a content-length: the body is not chunked");
        let mut body = buf[head_end..].to_vec();
        while body.len() < len {
            let n = sock.read(&mut chunk).await.unwrap();
            assert!(n > 0, "closed before the body ended");
            body.extend_from_slice(&chunk[..n]);
        }
        let reason = if status == 200 { "OK" } else { "Forbidden" };
        let answer = if status == 200 { "" } else { "<Error><Message>Signature expired</Message></Error>" };
        sock.write_all(
            format!("HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len()).as_bytes(),
        )
        .await
        .unwrap();
        (head, body)
    });
    (url, handle)
}

fn target(url: &str) -> UploadTarget {
    UploadTarget {
        name: "clip.mp4".into(),
        key: "work/f/1/clip.mp4".into(),
        put_url: url.into(),
        public_url: "https://cdn.databayt.org/work/f/1/clip.mp4".into(),
        content_type: "video/mp4".into(),
    }
}

#[test]
fn content_types_follow_the_extension() {
    assert_eq!(content_type_of("scene.MP4"), "video/mp4");
    assert_eq!(content_type_of("a.b.png"), "image/png");
    assert_eq!(content_type_of("classroom.blend"), "application/octet-stream");
    assert_eq!(content_type_of("noext"), "application/octet-stream");
}

#[test]
fn only_safe_regular_files_are_picked() {
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let h = home.path().canonicalize().unwrap();
    std::fs::create_dir_all(h.join("work/sub")).unwrap();
    std::fs::create_dir_all(h.join(".ssh")).unwrap();
    for f in ["work/clip.mp4", "work/sub/clip.mp4", "work/scene.blend", ".ssh/id_rsa"] {
        std::fs::write(h.join(f), b"12345").unwrap();
    }
    std::fs::write(outside.path().join("x.txt"), "x").unwrap();
    let roots = vec![h.clone()];
    let p = |rel: &str| h.join(rel).display().to_string();

    let ok = pick_files(&[p("work/clip.mp4"), p("work/scene.blend")], &roots).unwrap();
    assert_eq!(ok[0].file, UploadFile { name: "clip.mp4".into(), size: 5, content_type: "video/mp4".into() });
    assert_eq!(ok[1].file.content_type, "application/octet-stream");

    let refused = |paths: Vec<String>| matches!(pick_files(&paths, &roots), Err(TaskError::Invalid(_)));
    assert!(refused(vec![]), "nothing chosen");
    assert!(refused(vec![p("work")]), "a folder");
    assert!(refused(vec![p(".ssh/id_rsa")]), "a hidden folder");
    assert!(refused(vec![p("work/missing.mp4")]), "not found");
    assert!(refused(vec!["work/clip.mp4".into()]), "relative");
    assert!(refused(vec![outside.path().join("x.txt").display().to_string()]), "outside every root");
    assert!(refused(vec![p("work/clip.mp4"), p("work/sub/clip.mp4")]), "the same name twice");
    assert!(refused((0..21).map(|i| format!("/x/{i}")).collect()), "more than 20");
    // A symlink out of the home folder resolves outside and is refused.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path().join("x.txt"), h.join("work/link.txt")).unwrap();
        assert!(refused(vec![p("work/link.txt")]), "a link to outside");
    }
}

#[test]
fn the_wire_types_use_the_contract_names() {
    let req = UploadRequest {
        id: "databayt/hogwarts#7".into(),
        files: vec![UploadFile { name: "a.mp4".into(), size: 9, content_type: "video/mp4".into() }],
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({"id":"databayt/hogwarts#7","files":[{"name":"a.mp4","size":9,"contentType":"video/mp4"}]})
    );
    let response: UploadResponse = serde_json::from_value(json!({"uploads":[{
        "name":"a.mp4","key":"work/f/7/a.mp4","putUrl":"https://s/put","publicUrl":"https://cdn/a.mp4","contentType":"video/mp4"}]}))
    .unwrap();
    assert_eq!(response.uploads[0].put_url, "https://s/put");
    let ev = serde_json::to_value(SubmitEvent::Progress { index: 0, count: 2, name: "a".into(), sent: 5, total: 10 }).unwrap();
    assert_eq!(ev, json!({"kind":"progress","index":0,"count":2,"name":"a","sent":5,"total":10}));
}

#[test]
fn the_answer_is_checked_and_put_in_file_order() {
    let files = vec![
        UploadFile { name: "b.png".into(), size: 1, content_type: "image/png".into() },
        UploadFile { name: "a.mp4".into(), size: 1, content_type: "video/mp4".into() },
    ];
    let t = |n: &str, put: &str, public: &str| json!({"name":n,"key":format!("k/{n}"),"putUrl":put,"publicUrl":public,"contentType":""});
    let good = json!({"uploads":[t("a.mp4","https://s/a","https://cdn/a.mp4"), t("b.png","https://s/b","https://cdn/b.png")]});
    let out = targets_from(&good, &files).unwrap();
    assert_eq!(out.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["b.png", "a.mp4"]);
    assert_eq!(out[0].content_type, "image/png", "an empty content type falls back to the file's");
    // A refusal in a 200: ok false.
    let no = json!({"ok":false,"error":"not-holder","detail":"claim it first"});
    assert!(matches!(targets_from(&no, &files), Err(TaskError::Refused(m)) if m == "not-holder: claim it first"));
    // A missing link, a non-https link, a malformed answer.
    let missing = json!({"uploads":[t("a.mp4","https://s/a","https://cdn/a.mp4")]});
    assert!(matches!(targets_from(&missing, &files), Err(TaskError::Local(_))));
    let plain = json!({"uploads":[t("a.mp4","http://evil/a","https://cdn/a.mp4"), t("b.png","https://s/b","https://cdn/b.png")]});
    assert!(targets_from(&plain, &files).is_err());
    let plain_public = json!({"uploads":[t("a.mp4","https://s/a","http://cdn/a.mp4"), t("b.png","https://s/b","https://cdn/b.png")]});
    assert!(targets_from(&plain_public, &files).is_err());
    assert!(targets_from(&json!({"nope":1}), &files).is_err());
}

#[tokio::test]
async fn a_file_is_streamed_whole_hashed_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mp4");
    let bytes: Vec<u8> = (0..700_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let (url, server) = serve_put(200).await;
    let seen = Arc::new(AtomicU64::new(0));
    let calls = Arc::new(AtomicU64::new(0));
    let (s, c) = (seen.clone(), calls.clone());
    let progress: Arc<dyn Fn(u64) + Send + Sync> = Arc::new(move |sent| {
        s.store(sent, Ordering::SeqCst);
        c.fetch_add(1, Ordering::SeqCst);
    });
    let hash = put_file(&put_client().unwrap(), &target(&url), &path, bytes.len() as u64, progress).await.unwrap();
    assert_eq!(hash, sha256_hex(&bytes));
    let (head, body) = server.await.unwrap();
    assert_eq!(body, bytes, "the bytes arrive intact");
    let head = head.to_ascii_lowercase();
    assert!(head.starts_with("put /work/f/1/clip.mp4?sig=abc "), "{head}");
    assert!(head.contains("content-type: video/mp4") && head.contains("content-length: 700000"), "{head}");
    assert!(!head.contains("authorization"), "the machine token never goes to the link");
    assert_eq!(seen.load(Ordering::SeqCst), 700_000, "the last report is the whole file");
    assert!(calls.load(Ordering::SeqCst) >= 2, "a progress line moves");
}

#[tokio::test]
async fn a_refused_put_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mp4");
    std::fs::write(&path, b"abc").unwrap();
    let (url, server) = serve_put(403).await;
    let err = put_file(&put_client().unwrap(), &target(&url), &path, 3, Arc::new(|_| {})).await.unwrap_err();
    server.await.unwrap();
    let msg = err.to_string();
    assert!(msg.contains("403") && msg.contains("Signature expired") && msg.contains("clip.mp4"), "{msg}");
    // A link that is not https is never used.
    let bad = UploadTarget { put_url: "http://example.com/x".into(), ..target("") };
    assert!(put_file(&put_client().unwrap(), &bad, &path, 3, Arc::new(|_| {})).await.is_err());
}

struct Mock {
    upload: Result<String, (u16, String)>,
    receipt: Result<String, (u16, String)>,
    posts: Mutex<Vec<(String, String)>>,
}

impl Api for Mock {
    async fn get(&self, _path: &str) -> Result<String, ApiError> {
        Err(ApiError::Status { status: 404, body: String::new() })
    }
    async fn post(&self, path: &str, body: String) -> Result<String, ApiError> {
        self.posts.lock().unwrap().push((path.to_owned(), body));
        let r = if path.ends_with("/upload") { &self.upload } else { &self.receipt };
        r.clone().map_err(|(status, body)| ApiError::Status { status, body })
    }
}

fn notes() -> Notes {
    Notes {
        driver: "claude".into(),
        model: "human".into(),
        duration_ms: 1234,
        summary: "  Built the classroom scene.  ".into(),
        next: Some("  Sara: light it.  ".into()),
        formula: Some("video.order.blender".into()),
        stage: None,
    }
}

fn upload_answer(url: &str) -> String {
    json!({"uploads":[{"name":"clip.mp4","key":"work/f/1/clip.mp4","putUrl":url,"publicUrl":"https://cdn.databayt.org/work/f/1/clip.mp4","contentType":"video/mp4"}]})
        .to_string()
}

type Listener = Arc<dyn Fn(SubmitEvent) + Send + Sync>;

fn events() -> (Listener, Arc<Mutex<Vec<SubmitEvent>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let l = log.clone();
    (Arc::new(move |e| l.lock().unwrap().push(e)), log)
}

fn picked_file(dir: &Path, bytes: &[u8]) -> Vec<Picked> {
    let path = dir.join("clip.mp4");
    std::fs::write(&path, bytes).unwrap();
    pick_files(&[path.display().to_string()], &[dir.canonicalize().unwrap()]).unwrap()
}

#[tokio::test]
async fn a_submission_uploads_then_posts_the_receipt_with_links_and_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"the finished scene".to_vec();
    let picked = picked_file(dir.path(), &bytes);
    let (url, server) = serve_put(200).await;
    let api = Mock { upload: Ok(upload_answer(&url)), receipt: Ok(r#"{"ok":true}"#.into()), posts: Mutex::new(Vec::new()) };
    let (on_event, log) = events();
    let urls = submit_with(&api, &put_client().unwrap(), "databayt/hogwarts#7", &picked, &notes(), on_event).await.unwrap();
    server.await.unwrap();
    assert_eq!(urls, ["https://cdn.databayt.org/work/f/1/clip.mp4"]);

    let posts = api.posts.lock().unwrap();
    assert_eq!(posts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["/api/machine/upload", "/api/machine/receipt"]);
    let req: serde_json::Value = serde_json::from_str(&posts[0].1).unwrap();
    assert_eq!(req["id"], "databayt/hogwarts#7");
    assert_eq!(req["files"][0], json!({"name":"clip.mp4","size":bytes.len(),"contentType":"video/mp4"}));
    let receipt: serde_json::Value = serde_json::from_str(&posts[1].1).unwrap();
    assert_eq!(receipt["artifacts"], json!(["https://cdn.databayt.org/work/f/1/clip.mp4"]));
    assert_eq!(receipt["hashes"], json!([sha256_hex(&bytes)]));
    assert_eq!(receipt["summary"], "Built the classroom scene.");
    assert_eq!(receipt["next"], "Sara: light it.");
    assert_eq!(receipt["formula"], "video.order.blender");
    assert!(receipt.get("stage").is_none(), "an unknown stage is not sent");
    assert_eq!((receipt["driver"].as_str(), receipt["model"].as_str(), receipt["durationMs"].as_u64()), (Some("claude"), Some("human"), Some(1234)));

    let kinds: Vec<&'static str> = log
        .lock()
        .unwrap()
        .iter()
        .map(|e| match e {
            SubmitEvent::Prepared { .. } => "prepared",
            SubmitEvent::Progress { .. } => "progress",
            SubmitEvent::Uploaded { .. } => "uploaded",
            SubmitEvent::Posting => "posting",
        })
        .collect();
    assert_eq!(kinds.first(), Some(&"prepared"));
    assert_eq!(&kinds[kinds.len() - 2..], ["uploaded", "posting"]);
    assert!(kinds.contains(&"progress"));
}

#[tokio::test]
async fn a_refused_upload_or_receipt_shows_the_servers_reason() {
    let dir = tempfile::tempdir().unwrap();
    let picked = picked_file(dir.path(), b"x");
    let (on_event, _) = events();

    // The upload route refuses: nothing is put, nothing is posted after.
    let api = Mock {
        upload: Err((409, r#"{"ok":false,"error":"not-holder","detail":"you do not hold this order"}"#.into())),
        receipt: Ok(r#"{"ok":true}"#.into()),
        posts: Mutex::new(Vec::new()),
    };
    let err = submit_with(&api, &put_client().unwrap(), "databayt/hogwarts#7", &picked, &notes(), on_event.clone()).await.unwrap_err();
    assert!(matches!(&err, TaskError::Refused(m) if m == "not-holder: you do not hold this order"), "{err}");
    assert_eq!(api.posts.lock().unwrap().len(), 1);

    // Another 4xx with a reason reads the same way.
    let api = Mock { upload: Err((403, r#"{"error":"too-large","detail":"over 5 GB"}"#.into())), receipt: Ok(String::new()), posts: Mutex::new(Vec::new()) };
    let err = submit_with(&api, &put_client().unwrap(), "databayt/hogwarts#7", &picked, &notes(), on_event.clone()).await.unwrap_err();
    assert!(matches!(&err, TaskError::Refused(m) if m == "too-large: over 5 GB"), "{err}");

    // The files go up, then the receipt is refused: the reason shows.
    let (url, server) = serve_put(200).await;
    let api = Mock {
        upload: Ok(upload_answer(&url)),
        receipt: Err((409, r#"{"ok":false,"error":"held","detail":"someone else holds it"}"#.into())),
        posts: Mutex::new(Vec::new()),
    };
    let err = submit_with(&api, &put_client().unwrap(), "databayt/hogwarts#7", &picked, &notes(), on_event.clone()).await.unwrap_err();
    server.await.unwrap();
    assert!(matches!(&err, TaskError::Refused(m) if m == "held: someone else holds it"), "{err}");

    // A failed PUT stops before the receipt.
    let (url, server) = serve_put(403).await;
    let api = Mock { upload: Ok(upload_answer(&url)), receipt: Ok(r#"{"ok":true}"#.into()), posts: Mutex::new(Vec::new()) };
    let err = submit_with(&api, &put_client().unwrap(), "databayt/hogwarts#7", &picked, &notes(), on_event).await.unwrap_err();
    server.await.unwrap();
    assert!(err.to_string().contains("403"), "{err}");
    assert_eq!(api.posts.lock().unwrap().len(), 1, "no receipt after a failed upload");
}

#[tokio::test]
async fn no_summary_means_nothing_goes_on_the_wire() {
    let dir = tempfile::tempdir().unwrap();
    let picked = picked_file(dir.path(), b"x");
    let api = Mock { upload: Ok(String::new()), receipt: Ok(String::new()), posts: Mutex::new(Vec::new()) };
    let (on_event, _) = events();
    let mut n = notes();
    n.summary = "   ".into();
    let err = submit_with(&api, &put_client().unwrap(), "databayt/hogwarts#7", &picked, &n, on_event).await.unwrap_err();
    assert!(matches!(err, TaskError::Invalid(_)));
    assert!(api.posts.lock().unwrap().is_empty());
}

#[test]
fn a_receipt_with_hashes_must_have_one_per_artifact() {
    let good = receipt_for("databayt/kun#3", &notes(), vec!["https://c/a".into()], vec!["a".repeat(64)]);
    assert!(good.validate().is_ok());
    let few = receipt_for("databayt/kun#3", &notes(), vec!["https://c/a".into(), "https://c/b".into()], vec!["a".repeat(64)]);
    assert!(few.validate().is_err());
    let bad = receipt_for("databayt/kun#3", &notes(), vec!["https://c/a".into()], vec!["XYZ".into()]);
    assert!(bad.validate().is_err());
    let upper = receipt_for("databayt/kun#3", &notes(), vec!["https://c/a".into()], vec!["A".repeat(64)]);
    assert!(upper.validate().is_err(), "lowercase hex only");
    // Without hashes, formula and stage the wire shape of an older receipt is unchanged.
    let old = Receipt { formula: None, stage: None, hashes: None, ..good.clone() };
    let v = serde_json::to_value(&old).unwrap();
    assert!(v.get("formula").is_none() && v.get("stage").is_none() && v.get("hashes").is_none());
}
