//! providers — the Providers screen's back end: kun's catalog merged with its ledger, and the
//! few actions a row offers (save a key, test, set the pool up). The work itself is kun's: Machine
//! runs `omni.mjs`, `combos.mjs`, `harvest.mjs` and `omniroute.sh` with `<kun>` as the working
//! folder, each under a timeout, and turns a failure into one readable line.
//!
//! A key crosses from JS to Rust once, on submit. It is stored in the OS credential store under the
//! catalog's own account name (the bare name kun's scripts read) and never returned: the view only
//! says whether a provider is connected.

use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::pool::{self, CatalogProvider, Harvest};
use crate::{
    driver::{find_binary, search_dirs},
    profile::SecretStore,
};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
pub const TEST_TIMEOUT: Duration = Duration::from_secs(60);
pub const SETUP_TIMEOUT: Duration = Duration::from_secs(180);
/// `omniroute.sh install` waits up to a minute for the service to answer.
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(120);
const QUICK_TIMEOUT: Duration = Duration::from_secs(90);

/// A row of the Providers screen is one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "brain/types.ts")]
pub enum ProviderMode {
    /// A free source that takes a pasted key and joins the pool.
    Pool,
    /// A free source with no key to paste (signed in elsewhere, or keyless).
    Direct,
    /// Approved but off until funded; read-only.
    Paid,
    /// The person's subscription: shown, never offered to the pool.
    Plan,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
pub struct QuotaView {
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub remaining: Option<f64>,
    pub unit: Option<String>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
#[allow(clippy::struct_excessive_bools)] // a wire shape: one flag per thing the row's actions depend on
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub mode: ProviderMode,
    pub enabled: bool,
    /// The ledger's status word, or `unknown` before the first harvest.
    pub status: String,
    pub signup: Option<String>,
    /// A key is saved and the provider is connected to the gateway (never the key itself).
    pub key_set: bool,
    /// This provider also takes an account id (Cloudflare).
    pub needs_account: bool,
    /// There is a gateway connection to test.
    pub can_test: bool,
    pub requests: Option<f64>,
    pub success_pct: Option<f64>,
    pub avg_ms: Option<f64>,
    pub quota: Option<QuotaView>,
    pub note: Option<String>,
    /// Paid rows: the approval line.
    pub decide: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
pub struct ComboRow {
    pub name: String,
    pub ready: bool,
    pub members: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
pub struct ProvidersView {
    /// `ok`, `down` or `missing`: what answers on the gateway's port now.
    pub gateway: String,
    pub pool_ready: bool,
    pub updated: Option<String>,
    /// The ledger is older than two hours (or its age is unknown).
    pub stale: bool,
    /// kun's pool scripts and Node.js are here, so the actions can run.
    pub scripts_ready: bool,
    pub rows: Vec<ProviderRow>,
    pub combos: Vec<ComboRow>,
    /// Why there are no rows (no catalog), when that is the case.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
pub struct TestResult {
    pub ok: bool,
    pub latency_ms: Option<f64>,
    pub message: Option<String>,
}

// ---------------------------------------------------------------- the view

fn mode_of(p: &CatalogProvider) -> ProviderMode {
    if p.tier == "plan" || p.via == "claude" {
        ProviderMode::Plan
    } else if p.tier == "paid" {
        ProviderMode::Paid
    } else if p.enabled && p.keychain.is_some() && p.omniroute.is_some() {
        ProviderMode::Pool
    } else {
        ProviderMode::Direct
    }
}

fn row_of(p: &CatalogProvider, ledger: Option<&pool::Row>) -> ProviderRow {
    let mode = mode_of(p);
    let status = match ledger {
        Some(r) if !r.status.is_empty() => r.status.clone(),
        _ if !p.enabled => "off".to_owned(),
        _ => "unknown".to_owned(),
    };
    let key_set = mode == ProviderMode::Pool && !matches!(status.as_str(), "needs-key" | "unknown" | "off");
    let note = ledger
        .and_then(|r| r.note.clone())
        .or_else(|| p.note.clone())
        .or_else(|| (mode == ProviderMode::Direct).then(|| p.login.clone()).flatten());
    ProviderRow {
        id: p.id.clone(),
        name: p.name.clone(),
        tier: p.tier.clone(),
        mode,
        enabled: p.enabled,
        signup: p.signup.clone(),
        key_set,
        needs_account: mode == ProviderMode::Pool && p.account_keychain.is_some(),
        can_test: ledger.is_some_and(|r| r.connection.is_some()),
        requests: ledger.and_then(|r| r.requests),
        success_pct: ledger.and_then(|r| r.success_pct),
        avg_ms: ledger.and_then(|r| r.avg_ms),
        quota: ledger.and_then(|r| r.quota.as_ref()).map(|q| QuotaView {
            used: q.used,
            limit: q.limit,
            remaining: q.remaining,
            unit: q.unit.clone(),
            resets_at: q.resets_at.clone(),
        }),
        note,
        decide: (mode == ProviderMode::Paid).then(|| p.decide.clone()).flatten(),
        status,
    }
}

/// The catalog's rows in catalog order, each merged with its ledger row. Pure.
#[must_use]
pub fn build_view(
    catalog: &[CatalogProvider],
    harvest: Option<&Harvest>,
    age: Option<Duration>,
    gateway_answers: bool,
    scripts_ready: bool,
) -> ProvidersView {
    let rows = catalog
        .iter()
        .map(|p| row_of(p, harvest.and_then(|h| h.providers.iter().find(|r| r.id == p.id))))
        .collect();
    let gateway = if gateway_answers {
        "ok"
    } else if harvest.is_some_and(|h| h.gateway.status == "missing") {
        "missing"
    } else {
        "down"
    };
    let combos = harvest
        .map(|h| {
            h.combos
                .iter()
                .map(|(name, c)| ComboRow {
                    name: name.clone(),
                    ready: c.ready,
                    members: u32::try_from(c.members.len()).unwrap_or(u32::MAX),
                })
                .collect()
        })
        .unwrap_or_default();
    ProvidersView {
        gateway: gateway.to_owned(),
        pool_ready: harvest.is_some_and(|h| h.pool_ready),
        updated: harvest.and_then(|h| h.updated.clone()),
        stale: harvest.is_some() && age.is_none_or(|a| a > pool::STALE_AFTER),
        scripts_ready,
        rows,
        combos,
        error: None,
    }
}

// ---------------------------------------------------------------- running kun's scripts

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Node,
    Bash,
}

/// Runs one of kun's harvest scripts. The real one spawns a process; tests use a recording fake.
pub trait Runner {
    /// Runs `<kun>/.claude/scripts/harvest/<script> <args…>` and returns its stdout.
    ///
    /// # Errors
    /// One readable line: missing script or Node, a timeout, or the script's own last error line.
    fn run(&self, tool: Tool, script: &str, args: &[&str], timeout: Duration) -> Result<String, String>;
}

/// The harvest scripts' folder.
#[must_use]
pub fn scripts_dir(kun: &Path) -> PathBuf {
    kun.join(".claude/scripts/harvest")
}

/// The scripts the Providers actions need are all here.
#[must_use]
pub fn scripts_present(kun: &Path) -> bool {
    let dir = scripts_dir(kun);
    ["omni.mjs", "combos.mjs", "harvest.mjs", "omniroute.sh"].iter().all(|f| dir.join(f).is_file())
}

/// Runs scripts with `kun` as the working folder, `node` found the way the drivers are.
pub struct KunRunner {
    pub kun: PathBuf,
}

/// The last non-empty line of a process's output, trimmed to something a person can read.
fn last_line(text: &str) -> String {
    let line = text.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("it failed without a message");
    let line = line.strip_prefix("omni: ").or_else(|| line.strip_prefix("omniroute: ")).unwrap_or(line);
    line.chars().take(200).collect()
}

fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    })
}

impl Runner for KunRunner {
    fn run(&self, tool: Tool, script: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
        let path = scripts_dir(&self.kun).join(script);
        if !path.is_file() {
            return Err("kun has no pool scripts here; update the kun checkout".to_owned());
        }
        let dirs = search_dirs();
        let program = match tool {
            Tool::Node => find_binary("node", &dirs).ok_or("Node.js is not installed")?,
            Tool::Bash => PathBuf::from("/bin/bash"),
        };
        let mut path_dirs = dirs;
        path_dirs.dedup();
        let path_env = std::env::join_paths(path_dirs).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let mut child = Command::new(program)
            .arg(&path)
            .args(args)
            .current_dir(&self.kun)
            .env("PATH", path_env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not start {script}: {e}"))?;
        let out = child.stdout.take().map(drain);
        let err = child.stderr.take().map(drain);
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{script} timed out after {} s", timeout.as_secs()));
                }
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(format!("{script}: {e}")),
            }
        };
        let stdout = out.and_then(|h| h.join().ok()).unwrap_or_default();
        let stderr = err.and_then(|h| h.join().ok()).unwrap_or_default();
        if status.success() {
            Ok(stdout)
        } else {
            Err(last_line(if stderr.trim().is_empty() { &stdout } else { &stderr }))
        }
    }
}

/// The JSON object a script printed (log noise before it is skipped).
fn json_of(out: &str) -> Option<Value> {
    serde_json::from_str(out.trim())
        .ok()
        .or_else(|| out.find('{').and_then(|i| serde_json::from_str(out[i..].trim()).ok()))
}

// ---------------------------------------------------------------- the actions

fn catalog_row<'a>(catalog: &'a [CatalogProvider], id: &str) -> Result<&'a CatalogProvider, String> {
    catalog.iter().find(|p| p.id == id).ok_or_else(|| "that provider is not in kun's catalog".to_owned())
}

/// A pasted value kept only when it is plausible: trimmed, not empty, not huge, no control characters.
fn clean(value: &str) -> Option<&str> {
    let v = value.trim();
    (!v.is_empty() && v.len() <= 8192 && !v.chars().any(char::is_control)).then_some(v)
}

/// Rebuilds the combos and refreshes the ledger: the two steps after any change to the pool.
///
/// # Errors
/// The failing step, in one line.
pub fn refresh(runner: &impl Runner) -> Result<(), String> {
    runner.run(Tool::Node, "combos.mjs", &[], QUICK_TIMEOUT).map_err(|e| format!("combos: {e}"))?;
    runner.run(Tool::Node, "harvest.mjs", &["--quiet"], QUICK_TIMEOUT).map_err(|e| format!("harvest: {e}"))?;
    Ok(())
}

/// Stores `key` (and `account`) under the catalog's account names, then connects the provider,
/// rebuilds the combos and refreshes the ledger, in that order. Only a provider that takes a pasted
/// key is accepted: a paid row or the plan never is.
///
/// # Errors
/// An unknown or not-pasteable provider, a bad value, the store, or the failing step in one line.
pub fn save(
    store: &impl SecretStore,
    runner: &impl Runner,
    catalog: &[CatalogProvider],
    id: &str,
    key: &str,
    account: Option<&str>,
) -> Result<(), String> {
    let p = catalog_row(catalog, id)?;
    if mode_of(p) != ProviderMode::Pool {
        return Err(format!("{} does not take a pasted key", p.name));
    }
    let (Some(name), Some(key)) = (p.keychain.as_deref(), clean(key)) else {
        return Err("paste the key first".to_owned());
    };
    let account = account.filter(|a| !a.trim().is_empty());
    if let Some(a) = account {
        let (Some(slot), Some(a)) = (p.account_keychain.as_deref(), clean(a)) else {
            return Err("that account id is not valid".to_owned());
        };
        store.set(slot, a).map_err(|e| format!("could not save the account id: {e}"))?;
    }
    store.set(name, key).map_err(|e| format!("could not save the key: {e}"))?;
    let out = runner
        .run(Tool::Node, "omni.mjs", &["connect", id], CONNECT_TIMEOUT)
        .map_err(|e| format!("The key is saved, but connecting failed: {e}"))?;
    let status = json_of(&out).and_then(|v| v.get("status").and_then(Value::as_str).map(str::to_owned));
    match status.as_deref() {
        Some("added" | "updated") => {}
        Some("needs-key") => return Err("The key was not found in the credential store.".to_owned()),
        Some("needs-account-id") => return Err("The key is saved. This provider also needs its account id.".to_owned()),
        other => return Err(format!("The key is saved, but connecting failed ({}).", other.unwrap_or("no answer"))),
    }
    refresh(runner)
}

/// One upstream test of a connected provider.
///
/// # Errors
/// An unknown provider, none connected yet, or the script failing.
pub fn test(runner: &impl Runner, harvest: Option<&Harvest>, id: &str) -> Result<TestResult, String> {
    let connection = harvest
        .and_then(|h| h.providers.iter().find(|r| r.id == id))
        .and_then(|r| r.connection.clone())
        .ok_or_else(|| "this provider is not connected yet".to_owned())?;
    let out = runner.run(Tool::Node, "omni.mjs", &["test", &connection], TEST_TIMEOUT)?;
    let v = json_of(&out).ok_or_else(|| "the test printed nothing readable".to_owned())?;
    let ok = v.get("valid").and_then(Value::as_bool).unwrap_or(false);
    let latency_ms = v.get("latencyMs").and_then(Value::as_f64);
    let message = v
        .get("error")
        .and_then(Value::as_str)
        .or_else(|| v.pointer("/diagnosis/message").and_then(Value::as_str))
        .map(|m| m.chars().take(200).collect());
    Ok(TestResult { ok, latency_ms, message })
}

/// Installs the gateway service (loopback, required key, secrets), a no-op for kun when it is
/// already running. The launchd service is kun's: the app never stops it.
///
/// # Errors
/// The script's last error line.
pub fn install_gateway(runner: &impl Runner) -> Result<(), String> {
    runner.run(Tool::Bash, "omniroute.sh", &["install"], INSTALL_TIMEOUT).map(|_| ())
}

/// The one-tap fix: the gateway service, then every enabled provider and the combos, then the ledger.
///
/// # Errors
/// The failing step, in one line.
pub fn setup(runner: &impl Runner) -> Result<(), String> {
    install_gateway(runner)?;
    runner.run(Tool::Node, "omni.mjs", &["setup"], SETUP_TIMEOUT).map_err(|e| format!("setup: {e}"))?;
    runner.run(Tool::Node, "harvest.mjs", &["--quiet"], QUICK_TIMEOUT).map_err(|e| format!("harvest: {e}"))?;
    Ok(())
}

/// Creates the key clients use to talk to the gateway (it lands in the credential store, not here).
/// The gateway must be running to mint it, so a stopped one is installed first.
///
/// # Errors
/// The failing step, in one line.
pub fn mint_key(runner: &impl Runner, gateway_answers: bool) -> Result<(), String> {
    if !gateway_answers {
        install_gateway(runner)?;
    }
    runner.run(Tool::Node, "omni.mjs", &["mint-key"], CONNECT_TIMEOUT).map(|_| ()).map_err(|e| format!("mint-key: {e}"))
}

/// Opens `url` (http or https only) with the system opener.
///
/// # Errors
/// When the address is not http(s) or the opener fails.
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.chars().any(|c| c.is_control() || c == ' ') {
        return Err("not a web address".to_owned());
    }
    let mut cmd = if cfg!(target_os = "macos") {
        Command::new("/usr/bin/open")
    } else if cfg!(windows) {
        let mut c = Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        Command::new("xdg-open")
    };
    let status = cmd
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("could not open the browser: {e}"))?;
    if status.success() { Ok(()) } else { Err("the browser did not open".to_owned()) }
}
