//! pool — the token pool as Machine reads it. kun owns both files: the catalog
//! (`<kun>/.claude/machine/providers.json`, what a provider is) and the ledger
//! (`~/.claude/state/harvest.json`, what each one has, used and left, rewritten by
//! `harvest.mjs`). Machine only reads them; the schema is in `harvest.mjs`'s header.
//!
//! Everything here is pure or a plain file read, so the health check, the chat fallback and the
//! Providers screen share one reading of the ledger.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A harvest older than this is stale: the 30-minute job stopped running.
pub const STALE_AFTER: Duration = Duration::from_secs(2 * 60 * 60);

/// A number from JSON that may arrive as a number, a numeric string or null.
fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn lenient<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.as_ref().and_then(num))
}

/// A string from JSON that may arrive as a string or a number (a reset time as epoch seconds).
fn lenient_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) if !s.is_empty() => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

// ---------------------------------------------------------------- the catalog

/// One row of `providers.json`; fields Machine does not show are ignored.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CatalogProvider {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// `keyless`, `free`, `free-capped`, `paid` or `plan`.
    #[serde(default)]
    pub tier: String,
    /// `omniroute`, `direct`, `opencode`, `agy` or `claude`.
    #[serde(default)]
    pub via: String,
    /// The provider's id inside `OmniRoute`; absent for a source that never joins the pool.
    #[serde(default)]
    pub omniroute: Option<String>,
    #[serde(default)]
    pub signup: Option<String>,
    /// The account name of this provider's key in the credential store.
    #[serde(default)]
    pub keychain: Option<String>,
    /// The account name of its second value (Cloudflare's account id).
    #[serde(default)]
    pub account_keychain: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub note: Option<String>,
    /// Paid rows: who approved them and when they switch on.
    #[serde(default)]
    pub decide: Option<String>,
    /// How the provider is signed in when there is no key to paste.
    #[serde(default)]
    pub login: Option<String>,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    providers: Vec<CatalogProvider>,
}

/// `<kun>/.claude/machine/providers.json`.
#[must_use]
pub fn catalog_path(kun: &Path) -> PathBuf {
    kun.join(".claude/machine/providers.json")
}

/// Parses the catalog text.
///
/// # Errors
/// When the text is not the catalog's JSON.
pub fn parse_catalog(text: &str) -> Result<Vec<CatalogProvider>, String> {
    serde_json::from_str::<CatalogFile>(text)
        .map(|f| f.providers)
        .map_err(|e| format!("providers.json is damaged: {e}"))
}

/// Reads kun's catalog.
///
/// # Errors
/// When kun has no catalog here or it is damaged.
pub fn load_catalog(kun: &Path) -> Result<Vec<CatalogProvider>, String> {
    let path = catalog_path(kun);
    let text = fs::read_to_string(&path)
        .map_err(|_| "kun has no providers file here; update the kun checkout".to_owned())?;
    parse_catalog(&text)
}

// ---------------------------------------------------------------- the ledger

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Gateway {
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Quota {
    #[serde(default, deserialize_with = "lenient")]
    pub used: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub limit: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub remaining: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub resets_at: Option<String>,
}

/// One provider's row in `harvest.json`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Row {
    pub id: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// `connected`, `needs-key`, `needs-sign-in`, `exhausted`, `error`, `off`, `direct-ok`, `direct-down`.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub connection: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub requests: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub success_pct: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub avg_ms: Option<f64>,
    #[serde(default)]
    pub quota: Option<Quota>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Combo {
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub ready: bool,
}

/// `~/.claude/state/harvest.json`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Harvest {
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub gateway: Gateway,
    #[serde(default)]
    pub pool_ready: bool,
    #[serde(default)]
    pub providers: Vec<Row>,
    #[serde(default)]
    pub combos: BTreeMap<String, Combo>,
}

/// Where the ledger lives.
#[must_use]
pub fn harvest_path() -> PathBuf {
    crate::engine::claude_dir().join("state/harvest.json")
}

/// The ledger and how old the file is; `None` when it is missing or not the ledger's JSON.
#[must_use]
pub fn load_harvest(path: &Path) -> Option<(Harvest, Option<Duration>)> {
    let text = fs::read_to_string(path).ok()?;
    let harvest = serde_json::from_str::<Harvest>(&text).ok()?;
    let age = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok());
    Some((harvest, age))
}

// ---------------------------------------------------------------- the health item

/// How the pool stands, as the health check shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolLevel {
    Green,
    Yellow,
    Red,
}

impl PoolLevel {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Red => "red",
        }
    }
}

/// The level and one line saying why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolHealth {
    pub level: PoolLevel,
    /// What the heartbeat's `ok` says. kun's server paints a whole machine red from any item with
    /// `ok: false`, so this is false only when `OmniRoute` is installed and its gateway is down.
    /// A yellow or red light can still be `ok`.
    pub ok: bool,
    pub detail: String,
}

fn free_tier(tier: &str) -> bool {
    matches!(tier, "keyless" | "free" | "free-capped")
}

/// Green: the gateway answers and a provider is ready. Yellow: it works, but an enabled free
/// provider is exhausted or failing, or the ledger is older than two hours. Red: the gateway is
/// not ok, no provider is ready, or there is no ledger. Pure.
#[must_use]
pub fn health(harvest: Option<&Harvest>, age: Option<Duration>) -> PoolHealth {
    let red = |detail: &str| PoolHealth { level: PoolLevel::Red, ok: true, detail: detail.to_owned() };
    let Some(h) = harvest else {
        return red("no harvest yet: set up the token pool in Settings → Model → Providers");
    };
    if h.gateway.status != "ok" {
        return red(&format!("the gateway is {}", if h.gateway.status.is_empty() { "unknown" } else { &h.gateway.status }));
    }
    if !h.pool_ready {
        return red("no provider is ready: add a free key in Settings → Model → Providers");
    }
    let trouble: Vec<&str> = h
        .providers
        .iter()
        .filter(|p| p.enabled && free_tier(&p.tier) && matches!(p.status.as_str(), "exhausted" | "error"))
        .map(|p| p.id.as_str())
        .collect();
    if !trouble.is_empty() {
        return PoolHealth { level: PoolLevel::Yellow, ok: true, detail: format!("exhausted or failing: {}", trouble.join(", ")) };
    }
    if age.is_none_or(|a| a > STALE_AFTER) {
        return PoolHealth { level: PoolLevel::Yellow, ok: true, detail: "the harvest is more than 2 hours old".to_owned() };
    }
    PoolHealth { level: PoolLevel::Green, ok: true, detail: "gateway ok, pool ready".to_owned() }
}

/// The heartbeat's `ok`: false only when `OmniRoute` is installed and the gateway is down. Pure.
#[must_use]
pub fn heartbeat_ok(installed: bool, gateway_answers: bool) -> bool {
    !installed || gateway_answers
}

/// The health item for this computer: the light from the ledger file, `ok` from a live probe.
#[must_use]
pub fn current_health() -> PoolHealth {
    let loaded = load_harvest(&harvest_path());
    let mut out = match &loaded {
        Some((h, age)) => health(Some(h), *age),
        None => health(None, None),
    };
    let missing = loaded.as_ref().is_some_and(|(h, _)| h.gateway.status == "missing");
    let installed = !missing
        && (loaded.is_some() || crate::driver::find_binary("omniroute", &crate::driver::search_dirs()).is_some());
    let answers = super::Proxy::OmniRoute.spec(PathBuf::new()).is_up(Duration::from_millis(300));
    out.ok = heartbeat_ok(installed, answers);
    out
}

// ---------------------------------------------------------------- the chat fallback

/// The pool can serve a run now: the ledger says it is ready and the gateway answered a probe. Pure.
#[must_use]
pub fn usable(harvest: Option<&Harvest>, gateway_answers: bool) -> bool {
    gateway_answers && harvest.is_some_and(|h| h.pool_ready && h.gateway.status == "ok")
}
