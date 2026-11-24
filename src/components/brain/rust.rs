//! brain — which model runs the Claude Code CLI.
//!
//! Claude on the person's own sign-in is the default. Three opt-in free routes point the same CLI
//! at a local Anthropic-compatible proxy: NVIDIA and `OpenRouter` through Free Claude Code (`fcc-server`,
//! :8082, configured by `~/.fcc/.env`), and `OmniRoute` (:20128), the token pool: kun's launchd
//! service with its own model classes (`kun-code`, `kun-fast`, ...), each an ordered chain of free
//! models. The app starts a proxy when nothing answers, adopts one that does, and stops only one it
//! started when it quits (see [`supervise`]); kun's `OmniRoute` service is never ours to stop.
//! Providers (which keys feed the pool) are in [`providers`], the ledger they read in [`pool`].
//!
//! Keys live in the OS credential store; the UI only learns "set / not set". A chosen route that is
//! not ready fails the run with one line naming the missing step; it never falls back to Claude.

pub mod pool;
pub mod providers;
pub mod supervise;

use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, PoisonError},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;

use crate::{
    driver::{find_binary, search_dirs},
    profile::{self, OsStore, ProfileError, Resolved, SecretStore},
    settings::{self, SettingsError},
};
use providers::{ProvidersView, TestResult};
use supervise::{Spec, SuperviseError, SUPERVISOR};

/// Which model a run uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "brain/types.ts")]
pub enum Brain {
    /// The person's own Claude sign-in (default).
    #[default]
    Claude,
    /// NVIDIA's free Nemotron through Free Claude Code.
    Nvidia,
    /// `OpenRouter`'s free router through Free Claude Code.
    OpenRouter,
    /// `OmniRoute`, stacking every free tier.
    OmniRoute,
}

/// The local proxy a free route talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proxy {
    Fcc,
    OmniRoute,
}

pub const FCC_PORT: u16 = 8082;
pub const OMNIROUTE_PORT: u16 = 20128;
/// Free Claude Code takes any non-empty token while its own proxy auth is off.
const FCC_PLACEHOLDER_TOKEN: &str = "freecc";
const NVIDIA_MODEL: &str = "nvidia_nim/nvidia/nemotron-3-ultra-550b-a55b";
const OPENROUTER_MODEL: &str = "open_router/openrouter/free";
/// `OmniRoute`'s own `auto` also routes to the keyless reverse-engineered providers it pre-registers.
/// `kun-code` is kun's ranked chain over only the providers the person connected.
const OMNIROUTE_MODEL: &str = "kun-code";
/// The small / background class: Claude Code's haiku and subagent models.
const OMNIROUTE_FAST_MODEL: &str = "kun-fast";
/// How long a freshly started proxy gets to answer.
const START_WAIT: Duration = Duration::from_secs(20);

impl Brain {
    pub const ALL: [Brain; 4] = [Brain::Claude, Brain::Nvidia, Brain::OpenRouter, Brain::OmniRoute];

    #[must_use]
    pub fn is_free(self) -> bool {
        self != Brain::Claude
    }

    /// The keychain name of the key this route needs (Claude needs none).
    #[must_use]
    pub fn secret_name(self) -> Option<&'static str> {
        match self {
            Brain::Claude => None,
            Brain::Nvidia => Some("NVIDIA_NIM_API_KEY"),
            Brain::OpenRouter => Some("OPENROUTER_API_KEY"),
            Brain::OmniRoute => Some("OMNIROUTE_API_KEY"),
        }
    }

    /// The model shown in the UI and written as `MODEL` (empty for Claude).
    #[must_use]
    pub fn model(self) -> &'static str {
        match self {
            Brain::Claude => "",
            Brain::Nvidia => NVIDIA_MODEL,
            Brain::OpenRouter => OPENROUTER_MODEL,
            Brain::OmniRoute => OMNIROUTE_MODEL,
        }
    }

    #[must_use]
    pub fn proxy(self) -> Option<Proxy> {
        match self {
            Brain::Claude => None,
            Brain::Nvidia | Brain::OpenRouter => Some(Proxy::Fcc),
            Brain::OmniRoute => Some(Proxy::OmniRoute),
        }
    }

    /// The name shown in the UI and in notices.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Brain::Claude => "Claude",
            Brain::Nvidia => "NVIDIA",
            Brain::OpenRouter => "OpenRouter",
            Brain::OmniRoute => "OmniRoute",
        }
    }
}

impl Proxy {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Proxy::Fcc => "fcc",
            Proxy::OmniRoute => "omniroute",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Proxy::Fcc => "Free Claude Code",
            Proxy::OmniRoute => "OmniRoute",
        }
    }

    #[must_use]
    pub fn binary(self) -> &'static str {
        match self {
            Proxy::Fcc => "fcc-server",
            Proxy::OmniRoute => "omniroute",
        }
    }

    #[must_use]
    pub fn port(self) -> u16 {
        match self {
            Proxy::Fcc => FCC_PORT,
            Proxy::OmniRoute => OMNIROUTE_PORT,
        }
    }

    /// Only used when kun's service script is missing and the app starts `omniroute` itself.
    fn args(self) -> Vec<String> {
        match self {
            Proxy::Fcc => Vec::new(),
            Proxy::OmniRoute => vec!["serve".into(), "--headless".into()],
        }
    }

    /// Where the child runs. `OmniRoute` reads a `.env` from its working folder, so a bare start
    /// belongs in `~/.omniroute` (kun's service uses it), never in whatever folder the app was in.
    fn cwd(self) -> Option<PathBuf> {
        match self {
            Proxy::Fcc => None,
            Proxy::OmniRoute => dirs::home_dir().map(|h| h.join(".omniroute")),
        }
    }

    fn base_url(self) -> String {
        match self {
            Proxy::Fcc => format!("http://127.0.0.1:{FCC_PORT}"),
            Proxy::OmniRoute => format!("http://localhost:{OMNIROUTE_PORT}"),
        }
    }

    fn spec(self, program: PathBuf) -> Spec {
        Spec {
            name: self.name().to_owned(),
            program,
            args: self.args(),
            cwd: self.cwd(),
            env: vec![("PATH".into(), path_with_tools())],
            log: state_dir().join(format!("brain-{}.log", self.name())),
            port: self.port(),
            health_path: Some("/v1/models".into()),
        }
    }
}

/// The tool folders first, so a proxy started from a Finder launch still finds node or python.
fn path_with_tools() -> String {
    let mut dirs = search_dirs();
    dirs.dedup();
    std::env::join_paths(dirs)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn state_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude/state")
}

#[derive(Debug, thiserror::Error)]
pub enum BrainError {
    #[error("{0} is not installed. Install it in Settings → Model, then try again.")]
    NotInstalled(&'static str),
    #[error("No {0} key is saved. Add it in Settings → Model, then try again.")]
    NoKey(&'static str),
    #[error("The {title} proxy did not start ({detail}). Check Settings → Model.")]
    Proxy { title: &'static str, detail: String },
    #[error("could not find the home folder")]
    NoHome,
    #[error("{0}")]
    Io(#[from] io::Error),
    /// A Providers action failed; the text is one readable line.
    #[error("{0}")]
    Pool(String),
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
}

impl Serialize for BrainError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

// ---------------------------------------------------------------- ~/.fcc/.env

/// `KEY=value` with the value quoted so a key with spaces or quotes stays one value.
fn env_line(key: &str, value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./:+@".contains(c));
    if plain {
        format!("{key}={value}")
    } else {
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{key}=\"{escaped}\"")
    }
}

/// The key a line sets (`FOO=…` or `export FOO=…`), or `None` for a comment, a blank or text.
fn line_key(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with('#') {
        return None;
    }
    let t = t.strip_prefix("export ").map_or(t, str::trim_start);
    let (k, _) = t.split_once('=')?;
    let k = k.trim_end();
    (!k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(k)
}

/// Sets `updates` in a dotenv text: a line that already sets a key is replaced in place (later
/// duplicates of it are dropped), a missing key is appended, everything else — other keys, comments,
/// blank lines — is kept as it was. Pure.
#[must_use]
pub fn edit_env(existing: &str, updates: &[(&str, &str)]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for line in existing.lines() {
        match line_key(line).and_then(|k| updates.iter().find(|(n, _)| *n == k)) {
            Some((k, v)) => {
                if !seen.contains(k) {
                    seen.push(k);
                    out.push(env_line(k, v));
                }
            }
            None => out.push(line.to_owned()),
        }
    }
    for (k, v) in updates {
        if !seen.contains(k) {
            out.push(env_line(k, v));
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

fn fcc_env_path() -> Result<PathBuf, BrainError> {
    Ok(dirs::home_dir().ok_or(BrainError::NoHome)?.join(".fcc/.env"))
}

/// Writes `text` to `path` atomically; the file is private (0600) on unix because it holds keys.
fn write_private(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Points Free Claude Code at this route: the route's key and `MODEL` in `path`, nothing else touched.
/// Returns whether the file changed (a running server must then be restarted to read it).
///
/// # Errors
/// On a filesystem failure.
pub fn apply_fcc_env(path: &Path, brain: Brain, key: &str) -> io::Result<bool> {
    let Some(name) = brain.secret_name().filter(|_| brain.proxy() == Some(Proxy::Fcc)) else {
        return Ok(false);
    };
    let existing = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let next = edit_env(&existing, &[(name, key), ("MODEL", brain.model())]);
    if next == existing {
        return Ok(false);
    }
    write_private(path, &next)?;
    Ok(true)
}

// ---------------------------------------------------------------- the run's environment

/// The environment a run gets from a route: where Claude Code talks and as whom. Pure.
/// `OmniRoute` needs its own key as the token, `ANTHROPIC_MODEL=kun-code` for the main model and
/// `kun-fast` for the small and subagent models; Free Claude Code takes a placeholder token.
/// Claude adds nothing.
#[must_use]
pub fn route_env(brain: Brain, omniroute_key: Option<&str>) -> Vec<(String, String)> {
    let Some(proxy) = brain.proxy() else {
        return Vec::new();
    };
    let mut env = vec![("ANTHROPIC_BASE_URL".to_owned(), proxy.base_url())];
    match proxy {
        Proxy::Fcc => env.push(("ANTHROPIC_AUTH_TOKEN".into(), FCC_PLACEHOLDER_TOKEN.into())),
        Proxy::OmniRoute => {
            env.push(("ANTHROPIC_AUTH_TOKEN".into(), omniroute_key.unwrap_or_default().to_owned()));
            env.push(("ANTHROPIC_MODEL".into(), OMNIROUTE_MODEL.into()));
            env.push(("ANTHROPIC_DEFAULT_HAIKU_MODEL".into(), OMNIROUTE_FAST_MODEL.into()));
            env.push(("CLAUDE_CODE_SUBAGENT_MODEL".into(), OMNIROUTE_FAST_MODEL.into()));
        }
    }
    env
}

/// `env` with the route's variables in. Every other `ANTHROPIC_*` variable (an `ANTHROPIC_API_KEY`
/// among the profile's secrets would otherwise reach the proxy) and the telemetry switch are removed
/// first, as Free Claude Code's own launcher does. Pure.
#[must_use]
pub fn merge_env(env: Vec<(String, String)>, route: Vec<(String, String)>) -> Vec<(String, String)> {
    if route.is_empty() {
        return env;
    }
    let mut out: Vec<_> = env
        .into_iter()
        .filter(|(k, _)| {
            !k.starts_with("ANTHROPIC_") && k != "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC" && k != "CLAUDE_CODE_SUBAGENT_MODEL"
        })
        .collect();
    out.extend(route);
    out
}

/// The key of a route. Machine stores it under the profile's own name; kun's scripts (`omni.mjs
/// mint-key`, a key pasted in Providers) store the bare name, so that one counts too.
fn read_key(store: &impl SecretStore, resolved: &Resolved, name: &str) -> Result<Option<String>, ProfileError> {
    if let Some(v) = profile::read_secret(store, resolved, name)? {
        return Ok(Some(v));
    }
    Ok(store.get(name).map_err(ProfileError::Store)?.filter(|v| !v.trim().is_empty()))
}

/// Whether a route's key is saved, under either name. Never returns the value.
fn key_present(store: &impl SecretStore, resolved: &Resolved, name: &str) -> bool {
    profile::secret_present(store, resolved, name).unwrap_or(false) || store.get(name).ok().flatten().is_some_and(|v| !v.trim().is_empty())
}

/// Brings `OmniRoute` up when nothing answers. kun's service script wins when it is there: it binds
/// loopback, requires an API key and keeps its secrets (a bare `omniroute serve` outside
/// `~/.omniroute` binds 0.0.0.0 with no key), and the service is adopted, never stopped by the app.
/// Without the script the app starts `omniroute` itself, from `~/.omniroute`, and owns that child.
fn ensure_omniroute() -> Result<(), BrainError> {
    let proxy = Proxy::OmniRoute;
    let err = |detail: String| BrainError::Proxy { title: proxy.title(), detail };
    let probe = proxy.spec(PathBuf::new());
    if probe.is_up(Duration::from_millis(400)) {
        return Ok(());
    }
    let kun = crate::engine::kun_dir();
    if providers::scripts_present(&kun) {
        providers::install_gateway(&providers::KunRunner { kun }).map_err(err)?;
        let deadline = std::time::Instant::now() + START_WAIT;
        while !probe.is_up(Duration::from_millis(400)) {
            if std::time::Instant::now() >= deadline {
                return Err(err("it did not answer after install".to_owned()));
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        return Ok(());
    }
    let program = find_binary(proxy.binary(), &search_dirs()).ok_or(BrainError::NotInstalled(proxy.title()))?;
    if let Some(dir) = proxy.cwd() {
        fs::create_dir_all(dir)?;
    }
    SUPERVISOR
        .ensure(&proxy.spec(program), START_WAIT)
        .map(|_| ())
        .map_err(|e: SuperviseError| err(e.to_string()))
}

/// Makes a free route ready for a run and returns its environment additions: checks the proxy is
/// installed and the key saved, points Free Claude Code at the route, and starts or adopts the
/// proxy. Blocking (up to about 20 s when the proxy has to start, longer when kun's service is
/// installed). Claude returns nothing.
///
/// # Errors
/// One clear [`BrainError`] naming the missing step.
pub fn prepare(
    brain: Brain,
    resolved: &Resolved,
    store: &impl SecretStore,
) -> Result<Vec<(String, String)>, BrainError> {
    let (Some(proxy), Some(name)) = (brain.proxy(), brain.secret_name()) else {
        return Ok(Vec::new());
    };
    if proxy == Proxy::OmniRoute {
        let key = read_key(store, resolved, name)?.ok_or(BrainError::NoKey(brain.label()))?;
        ensure_omniroute()?;
        return Ok(route_env(brain, Some(key.as_str())));
    }
    let program = find_binary(proxy.binary(), &search_dirs()).ok_or(BrainError::NotInstalled(proxy.title()))?;
    let key = profile::read_secret(store, resolved, name)?.ok_or(BrainError::NoKey(brain.label()))?;
    if apply_fcc_env(&fcc_env_path()?, brain, &key)? {
        // A server we started read the old file: restart it. (One we merely adopted is left alone.)
        SUPERVISOR.stop(proxy.name());
    }
    SUPERVISOR
        .ensure(&proxy.spec(program), START_WAIT)
        .map_err(|e: SuperviseError| BrainError::Proxy { title: proxy.title(), detail: e.to_string() })?;
    Ok(route_env(brain, None))
}

// ---------------------------------------------------------------- status and commands

/// One route as the Model page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
pub struct RouteStatus {
    pub route: Brain,
    /// The program the route needs is on this computer.
    pub installed: bool,
    /// The route's key is saved (Claude needs none, so it is always true).
    pub key_set: bool,
    /// Something answers on the route's proxy port now.
    pub proxy_up: bool,
    pub model: String,
}

/// The saved choice plus every route's status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "brain/types.ts")]
pub struct BrainView {
    pub current: Brain,
    pub routes: Vec<RouteStatus>,
}

/// Where each route stands. Blocking (filesystem, keychain, a few loopback probes).
///
/// # Errors
/// When the profile or settings cannot be read.
pub fn view(store: &impl SecretStore) -> Result<BrainView, BrainError> {
    let resolved = profile::current()?;
    let current = settings::load()?.brain;
    let dirs = search_dirs();
    let probe = Duration::from_millis(300);
    let routes = Brain::ALL
        .into_iter()
        .map(|route| {
            let proxy = route.proxy();
            let installed = match proxy {
                Some(p) => find_binary(p.binary(), &dirs).is_some(),
                None => find_binary("claude", &dirs).is_some(),
            };
            let key_set = match route.secret_name() {
                Some(n) => key_present(store, &resolved, n),
                None => true,
            };
            let proxy_up = proxy.is_none_or(|p| p.spec(PathBuf::new()).is_up(probe));
            RouteStatus { route, installed, key_set, proxy_up, model: route.model().to_owned() }
        })
        .collect();
    Ok(BrainView { current, routes })
}

impl Spec {
    fn is_up(&self, timeout: Duration) -> bool {
        supervise::is_up(self, timeout)
    }
}

/// Saves the choice. A free route must be installed and have its key first; Free Claude Code's
/// `~/.fcc/.env` is pointed at it right away. The proxy itself starts with the next run.
///
/// # Errors
/// [`BrainError::NotInstalled`] / [`BrainError::NoKey`], or a file failure.
pub fn apply(route: Brain, store: &impl SecretStore) -> Result<BrainView, BrainError> {
    if let (Some(proxy), Some(name)) = (route.proxy(), route.secret_name()) {
        let resolved = profile::current()?;
        find_binary(proxy.binary(), &search_dirs()).ok_or(BrainError::NotInstalled(proxy.title()))?;
        let key = read_key(store, &resolved, name)?.ok_or(BrainError::NoKey(route.label()))?;
        if proxy == Proxy::Fcc && apply_fcc_env(&fcc_env_path()?, route, &key)? {
            SUPERVISOR.stop(proxy.name());
        }
    }
    let mut s = settings::load()?;
    s.brain = route;
    settings::save_in(&settings::machine_dir()?, s)?;
    view(store)
}

// ---------------------------------------------------------------- fallback when Claude is unavailable

/// The order free routes are tried when Claude is unavailable: `OmniRoute` stacks every free tier,
/// then NVIDIA, then `OpenRouter`.
pub const FALLBACK_ORDER: [Brain; 3] = [Brain::OmniRoute, Brain::Nvidia, Brain::OpenRouter];

/// The free routes that are ready. A route is ready when its program is installed and its key
/// saved. When the pool is usable (`pool_usable`: the ledger says it is ready and the gateway
/// answers) `OmniRoute` is first and ready on its key alone, since kun's service, not an
/// app-started child, is what answers. When it is not usable `OmniRoute` goes last, after NVIDIA
/// and `OpenRouter`, so a pool that cannot serve is the final resort. Pure.
#[must_use]
pub fn ready_fallbacks(routes: &[RouteStatus], pool_usable: bool) -> Vec<Brain> {
    let ready = |b: Brain, need_program: bool| routes.iter().any(|r| r.route == b && (r.installed || !need_program) && r.key_set);
    if pool_usable {
        let first = ready(Brain::OmniRoute, false).then_some(Brain::OmniRoute);
        first.into_iter().chain(FALLBACK_ORDER.into_iter().filter(|b| Some(*b) != first && ready(*b, true))).collect()
    } else {
        let (omni, rest): (Vec<Brain>, Vec<Brain>) =
            FALLBACK_ORDER.into_iter().filter(|b| ready(*b, true)).partition(|b| *b == Brain::OmniRoute);
        rest.into_iter().chain(omni).collect()
    }
}

/// Whether the token pool can serve a run right now: `harvest.json` says it is ready and the
/// gateway answers a probe. Blocking (one loopback probe).
#[must_use]
pub fn pool_usable() -> bool {
    let answers = Proxy::OmniRoute.spec(PathBuf::new()).is_up(Duration::from_millis(300));
    let loaded = pool::load_harvest(&pool::harvest_path());
    pool::usable(loaded.as_ref().map(|(h, _)| h), answers)
}

/// Sessions that fell back this app run: session id to the free route it is on. Memory only; a new
/// session always tries Claude first.
static FALLBACKS: LazyLock<Mutex<HashMap<String, Brain>>> = LazyLock::new(Mutex::default);

/// Remembers that `session` runs on `route` for the rest of the session.
pub fn fallback_set(session: &str, route: Brain) {
    FALLBACKS.lock().unwrap_or_else(PoisonError::into_inner).insert(session.to_owned(), route);
}

/// The route `session` fell back to, if it did.
#[must_use]
pub fn fallback_get(session: &str) -> Option<Brain> {
    FALLBACKS.lock().unwrap_or_else(PoisonError::into_inner).get(session).copied()
}

/// `brain_fallback_state` — the free route `session` fell back to because Claude was unavailable,
/// or null when it is on its chosen route. The model chip shows "(fallback)" from this.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // a Tauri command takes owned arguments
fn brain_fallback_state(session: String) -> Option<Brain> {
    fallback_get(&session)
}

/// `brain_status` — the saved route and where each one stands (installed, key, proxy, model).
#[tauri::command]
async fn brain_status() -> Result<BrainView, BrainError> {
    tokio::task::spawn_blocking(|| view(&OsStore))
        .await
        .map_err(|e| BrainError::Io(io::Error::other(e.to_string())))?
}

/// `brain_apply` — makes `route` the model for runs, after checking it is installed and has its key.
#[tauri::command]
async fn brain_apply(route: Brain) -> Result<BrainView, BrainError> {
    tokio::task::spawn_blocking(move || apply(route, &OsStore))
        .await
        .map_err(|e| BrainError::Io(io::Error::other(e.to_string())))?
}

/// Runs blocking work off the async runtime.
async fn blocking<T: Send + 'static>(job: impl FnOnce() -> Result<T, BrainError> + Send + 'static) -> Result<T, BrainError> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|e| BrainError::Io(io::Error::other(e.to_string())))?
}

fn pool_err(e: String) -> BrainError {
    BrainError::Pool(e)
}

fn gateway_answers() -> bool {
    Proxy::OmniRoute.spec(PathBuf::new()).is_up(Duration::from_millis(300))
}

/// The Providers screen: kun's catalog merged with its ledger, plus what answers on the gateway now.
fn providers_now() -> ProvidersView {
    let kun = crate::engine::kun_dir();
    let loaded = pool::load_harvest(&pool::harvest_path());
    let scripts = providers::scripts_present(&kun) && find_binary("node", &search_dirs()).is_some();
    match pool::load_catalog(&kun) {
        Ok(catalog) => providers::build_view(&catalog, loaded.as_ref().map(|(h, _)| h), loaded.as_ref().and_then(|(_, a)| *a), gateway_answers(), scripts),
        Err(e) => ProvidersView { error: Some(e), ..providers::build_view(&[], None, None, gateway_answers(), scripts) },
    }
}

/// `providers_status` — every provider in kun's catalog with what the ledger knows of it.
#[tauri::command]
async fn providers_status() -> Result<ProvidersView, BrainError> {
    blocking(|| Ok(providers_now())).await
}

/// `providers_save` — stores a pasted key (and account id) in the credential store, connects the
/// provider to the gateway, rebuilds the combos and refreshes the ledger. The key never comes back.
#[tauri::command]
async fn providers_save(id: String, key: String, account: Option<String>) -> Result<ProvidersView, BrainError> {
    blocking(move || {
        let kun = crate::engine::kun_dir();
        let catalog = pool::load_catalog(&kun).map_err(pool_err)?;
        providers::save(&OsStore, &providers::KunRunner { kun }, &catalog, &id, &key, account.as_deref()).map_err(pool_err)?;
        Ok(providers_now())
    })
    .await
}

/// `providers_test` — one upstream test of a connected provider: ok and its latency.
#[tauri::command]
async fn providers_test(id: String) -> Result<TestResult, BrainError> {
    blocking(move || {
        let loaded = pool::load_harvest(&pool::harvest_path());
        providers::test(&providers::KunRunner { kun: crate::engine::kun_dir() }, loaded.as_ref().map(|(h, _)| h), &id).map_err(pool_err)
    })
    .await
}

/// `providers_open` — opens a provider's sign-up page (the address comes from kun's catalog, never from JS).
#[tauri::command]
async fn providers_open(id: String) -> Result<(), BrainError> {
    blocking(move || {
        let catalog = pool::load_catalog(&crate::engine::kun_dir()).map_err(pool_err)?;
        let url = catalog.iter().find(|p| p.id == id).and_then(|p| p.signup.clone()).ok_or_else(|| pool_err("that provider has no sign-up page".to_owned()))?;
        providers::open_url(&url).map_err(pool_err)
    })
    .await
}

/// `providers_setup` — the gateway service, every enabled provider with a key, the combos, the ledger.
#[tauri::command]
async fn providers_setup() -> Result<ProvidersView, BrainError> {
    blocking(|| {
        providers::setup(&providers::KunRunner { kun: crate::engine::kun_dir() }).map_err(pool_err)?;
        Ok(providers_now())
    })
    .await
}

/// `brain_mint_key` — creates the gateway key `OmniRoute` runs need (`omni.mjs mint-key`), then
/// returns the refreshed Model page.
#[tauri::command]
async fn brain_mint_key() -> Result<BrainView, BrainError> {
    blocking(|| {
        providers::mint_key(&providers::KunRunner { kun: crate::engine::kun_dir() }, gateway_answers()).map_err(pool_err)?;
        view(&OsStore)
    })
    .await
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("brain")
        .invoke_handler(tauri::generate_handler![
            brain_status,
            brain_apply,
            brain_fallback_state,
            brain_mint_key,
            providers_status,
            providers_save,
            providers_test,
            providers_open,
            providers_setup
        ])
        .on_event(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                SUPERVISOR.stop_all();
            }
        })
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
