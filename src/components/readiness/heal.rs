//! readiness — the heal attempt cap: a failing auto step is retried by the health pass at most
//! three times per 24 hours, so a step that cannot be fixed never loops. The counts live in the
//! app data dir (`heal-attempts.json`), never in the heartbeat.

use std::{collections::BTreeMap, fs, io, path::Path};

/// Attempts a step gets per window.
pub(super) const MAX_ATTEMPTS: usize = 3;
/// The window, in seconds.
pub(super) const WINDOW_SECS: u64 = 24 * 60 * 60;

/// Step id → unix seconds of each attempt in the window.
pub(super) type Ledger = BTreeMap<String, Vec<u64>>;

/// A missing or unreadable ledger is an empty one.
pub(super) fn load(path: &Path) -> Ledger {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn recent(times: &[u64], now: u64) -> usize {
    times
        .iter()
        .filter(|t| now.saturating_sub(**t) < WINDOW_SECS)
        .count()
}

/// Whether the step may be tried again now.
pub(super) fn under_cap(ledger: &Ledger, id: &str, now: u64) -> bool {
    ledger.get(id).is_none_or(|t| recent(t, now) < MAX_ATTEMPTS)
}

/// Adds an attempt, dropping those older than the window, and writes the ledger atomically.
///
/// # Errors
/// When the file cannot be written.
pub(super) fn record(path: &Path, id: &str, now: u64) -> io::Result<()> {
    let mut ledger = load(path);
    for times in ledger.values_mut() {
        times.retain(|t| now.saturating_sub(*t) < WINDOW_SECS);
    }
    ledger.retain(|_, t| !t.is_empty());
    ledger.entry(id.to_owned()).or_default().push(now);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(&ledger).unwrap_or_else(|_| "{}".into()),
    )?;
    fs::rename(&tmp, path)
}

/// The failing steps the health pass may fix on its own: in the catalog and applicable here,
/// `fix.kind` auto, not human, and still under the attempt cap. In catalog order.
pub(super) fn candidates(
    catalog: &super::model::Catalog,
    scope: &super::catalog::Scope,
    state: &super::model::ReadinessState,
    ledger: &Ledger,
    now: u64,
) -> Vec<String> {
    super::catalog::ordered(catalog, scope)
        .into_iter()
        .filter(|s| state.failing.contains(&s.id))
        .filter(|s| {
            !s.human
                && s.fix
                    .as_ref()
                    .is_some_and(|f| f.kind == super::model::FixKind::Auto)
        })
        .filter(|s| under_cap(ledger, &s.id, now))
        .map(|s| s.id)
        .collect()
}
