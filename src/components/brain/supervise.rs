//! A supervised child: a local server the app starts when nothing answers, adopts when something
//! already does, and stops on exit only when it started it. Generic on purpose — the free-model
//! proxies use it, and a later pool donor can run its jobs the same way.
//!
//! The child is detached from the app's output (stdout and stderr go to a log file) and is never
//! waited on in the background; [`Supervisor::stop_all`] is the one place that kills.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{LazyLock, Mutex, PoisonError},
    time::{Duration, Instant},
};

/// What to run and where it will answer.
#[derive(Debug, Clone)]
pub struct Spec {
    /// Key under which the child is tracked (`fcc`, `omniroute`, …).
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Added to the child's environment.
    pub env: Vec<(String, String)>,
    /// stdout and stderr land here (created with its folder, appended to).
    pub log: PathBuf,
    pub port: u16,
    /// A GET path that answers once the server is ready (`/v1/models`); `None` is a bare port check.
    pub health_path: Option<String>,
}

/// What `ensure` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Something already answered; we did not start it and will not stop it.
    Adopted,
    /// We started it.
    Started,
}

#[derive(Debug, thiserror::Error)]
pub enum SuperviseError {
    #[error("could not start {0}: {1}")]
    Spawn(String, String),
    #[error("{0} exited before it answered (see {1})")]
    Exited(String, String),
    #[error("{0} did not answer on port {1} within {2} s (see {3})")]
    Timeout(String, u16, u64, String),
}

/// Whether something accepts a connection on the loopback port.
#[must_use]
pub fn port_open(port: u16, timeout: Duration) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, timeout).is_ok()
}

/// The HTTP status of `GET <path>` on the loopback port, or `None` when it does not answer HTTP.
#[must_use]
pub fn http_status(port: u16, path: &str, timeout: Duration) -> Option<u16> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?;
    s.set_write_timeout(Some(timeout)).ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: */*\r\n\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut head = [0u8; 32];
    let mut n = 0;
    while n < head.len() {
        match s.read(&mut head[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    parse_status(&String::from_utf8_lossy(&head[..n]))
}

/// `HTTP/1.1 200 OK` → 200.
fn parse_status(head: &str) -> Option<u16> {
    let mut parts = head.split_whitespace();
    if !parts.next()?.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse().ok()
}

/// Up means the port answers and, when there is a health path, it answers HTTP below 500
/// (a 401 from a server that wants a key is still a server that is up).
#[must_use]
pub fn is_up(spec: &Spec, timeout: Duration) -> bool {
    if !port_open(spec.port, timeout) {
        return false;
    }
    match &spec.health_path {
        Some(p) => http_status(spec.port, p, timeout).is_some_and(|s| s < 500),
        None => true,
    }
}

/// The children this app started, by name.
#[derive(Default)]
pub struct Supervisor {
    owned: Mutex<HashMap<String, Child>>,
}

/// The app-wide supervisor.
pub static SUPERVISOR: LazyLock<Supervisor> = LazyLock::new(Supervisor::default);

impl Supervisor {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Child>> {
        self.owned.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a child of this name is one we started and that is still running.
    pub fn owns(&self, name: &str) -> bool {
        let mut owned = self.lock();
        match owned.get_mut(name).map(Child::try_wait) {
            Some(Ok(None)) => true,
            Some(_) => {
                owned.remove(name);
                false
            }
            None => false,
        }
    }

    /// Adopts a server that already answers, else starts one and waits up to `wait` for it.
    ///
    /// # Errors
    /// When the program cannot start, exits early, or never answers (the child is killed then).
    pub fn ensure(&self, spec: &Spec, wait: Duration) -> Result<Outcome, SuperviseError> {
        let probe = Duration::from_millis(400);
        if is_up(spec, probe) {
            return Ok(Outcome::Adopted);
        }
        // An owned child that is up but not answering is stuck: replace it.
        self.stop(&spec.name);
        let log_name = spec.log.display().to_string();
        let child = spawn(spec).map_err(|e| SuperviseError::Spawn(spec.name.clone(), e))?;
        self.lock().insert(spec.name.clone(), child);
        let deadline = Instant::now() + wait;
        loop {
            if is_up(spec, probe) {
                return Ok(Outcome::Started);
            }
            let exited = self
                .lock()
                .get_mut(&spec.name)
                .is_none_or(|c| !matches!(c.try_wait(), Ok(None)));
            if exited {
                self.lock().remove(&spec.name);
                return Err(SuperviseError::Exited(spec.name.clone(), log_name));
            }
            if Instant::now() >= deadline {
                self.stop(&spec.name);
                return Err(SuperviseError::Timeout(spec.name.clone(), spec.port, wait.as_secs(), log_name));
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    /// Kills the child of that name if we started it. Returns whether there was one.
    pub fn stop(&self, name: &str) -> bool {
        let child = self.lock().remove(name);
        child.is_some_and(|mut c| {
            let _ = c.kill();
            let _ = c.wait();
            true
        })
    }

    /// Kills every child we started (app exit). A server we only adopted is left running.
    pub fn stop_all(&self) {
        let all: Vec<Child> = self.lock().drain().map(|(_, c)| c).collect();
        for mut c in all {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn open_log(path: &PathBuf) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

fn spawn(spec: &Spec) -> Result<Child, String> {
    let log = open_log(&spec.log).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group: a Ctrl-C in the terminal that started the app does not reach it.
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn().map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "supervise_test.rs"]
mod tests;
