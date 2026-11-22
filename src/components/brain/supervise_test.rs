use std::net::TcpListener;

use super::*;

fn spec(port: u16, program: &str, args: &[&str], log: PathBuf) -> Spec {
    Spec {
        name: "t".into(),
        program: program.into(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
        env: Vec::new(),
        log,
        port,
        health_path: None,
    }
}

#[test]
fn parses_a_status_line() {
    assert_eq!(parse_status("HTTP/1.1 200 OK\r\n"), Some(200));
    assert_eq!(parse_status("HTTP/1.0 401 Unauthorized"), Some(401));
    assert_eq!(parse_status("garbage"), None);
    assert_eq!(parse_status(""), None);
}

#[test]
fn an_open_port_is_adopted_and_never_owned() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let d = tempfile::tempdir().unwrap();
    let sup = Supervisor::default();
    let s = spec(port, "/nonexistent/never-run", &[], d.path().join("x.log"));
    assert_eq!(sup.ensure(&s, Duration::from_millis(200)).unwrap(), Outcome::Adopted);
    assert!(!sup.owns("t"));
}

#[test]
fn a_closed_port_with_a_missing_program_is_a_spawn_error() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let d = tempfile::tempdir().unwrap();
    let sup = Supervisor::default();
    let s = spec(port, "/nonexistent/never-run", &[], d.path().join("x.log"));
    assert!(matches!(sup.ensure(&s, Duration::from_millis(200)), Err(SuperviseError::Spawn(..))));
}

#[cfg(unix)]
#[test]
fn a_child_that_exits_early_is_reported_with_its_log() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let d = tempfile::tempdir().unwrap();
    let sup = Supervisor::default();
    let s = spec(port, "sh", &["-c", "echo boom; exit 3"], d.path().join("logs/x.log"));
    assert!(matches!(sup.ensure(&s, Duration::from_secs(5)), Err(SuperviseError::Exited(..))));
    assert!(fs::read_to_string(d.path().join("logs/x.log")).unwrap().contains("boom"));
    assert!(!sup.owns("t"));
}

#[cfg(unix)]
#[test]
fn a_child_that_never_answers_is_killed_on_timeout() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let d = tempfile::tempdir().unwrap();
    let sup = Supervisor::default();
    let s = spec(port, "sleep", &["30"], d.path().join("x.log"));
    assert!(matches!(sup.ensure(&s, Duration::from_millis(400)), Err(SuperviseError::Timeout(..))));
    assert!(!sup.owns("t"));
}

#[cfg(unix)]
#[test]
fn stop_all_kills_what_we_started() {
    let d = tempfile::tempdir().unwrap();
    let sup = Supervisor::default();
    let child = spawn(&spec(1, "sleep", &["30"], d.path().join("x.log"))).unwrap();
    sup.lock().insert("t".into(), child);
    assert!(sup.owns("t"));
    sup.stop_all();
    assert!(!sup.owns("t"));
}
