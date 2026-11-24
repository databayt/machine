use std::{cell::RefCell, collections::HashMap};

use super::*;

fn s(env: &[(&str, &str)]) -> Vec<(String, String)> {
    env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

#[test]
fn editor_updates_in_place_and_keeps_everything_else() {
    let before = "# my config\nexport NVIDIA_NIM_API_KEY=old\n\nPORT=9000\nMODEL=\"x\"\n";
    let after = edit_env(before, &[("NVIDIA_NIM_API_KEY", "nvapi-abc"), ("MODEL", "nvidia_nim/m")]);
    assert_eq!(after, "# my config\nNVIDIA_NIM_API_KEY=nvapi-abc\n\nPORT=9000\nMODEL=nvidia_nim/m\n");
}

#[test]
fn editor_inserts_missing_keys_and_creates_from_empty() {
    assert_eq!(
        edit_env("", &[("OPENROUTER_API_KEY", "k"), ("MODEL", "open_router/openrouter/free")]),
        "OPENROUTER_API_KEY=k\nMODEL=open_router/openrouter/free\n"
    );
    assert_eq!(edit_env("A=1", &[("B", "2")]), "A=1\nB=2\n");
}

#[test]
fn editor_ignores_comments_drops_duplicates_and_quotes_odd_values() {
    let out = edit_env("# MODEL=nope\nMODEL=a\nMODEL=b\n", &[("MODEL", "c")]);
    assert_eq!(out, "# MODEL=nope\nMODEL=c\n");
    assert_eq!(edit_env("", &[("K", "a b\"c")]), "K=\"a b\\\"c\"\n");
    // Idempotent: applying twice changes nothing.
    let twice = edit_env(&out, &[("MODEL", "c")]);
    assert_eq!(twice, out);
}

#[test]
fn apply_writes_a_private_file_and_reports_change() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join(".fcc/.env");
    assert!(apply_fcc_env(&path, Brain::Nvidia, "nvapi-1").unwrap());
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("NVIDIA_NIM_API_KEY=nvapi-1") && text.contains(&format!("MODEL={NVIDIA_MODEL}")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert!(!apply_fcc_env(&path, Brain::Nvidia, "nvapi-1").unwrap(), "same content, no change");
    // Switching route updates MODEL and keeps the other key.
    assert!(apply_fcc_env(&path, Brain::OpenRouter, "or-1").unwrap());
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("NVIDIA_NIM_API_KEY=nvapi-1") && text.contains("OPENROUTER_API_KEY=or-1"));
    assert!(text.contains(&format!("MODEL={OPENROUTER_MODEL}")) && !text.contains(NVIDIA_MODEL));
    // OmniRoute and Claude never touch the file.
    assert!(!apply_fcc_env(&path, Brain::OmniRoute, "k").unwrap());
}

#[test]
fn old_settings_files_load_as_claude() {
    let old = r#"{"person":"x","department":"","repo_by_department":{},"driver":"claude","login":"","autoRun":false}"#;
    let s: settings::Settings = serde_json::from_str(old).unwrap();
    assert_eq!(s.brain, Brain::Claude);
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("settings.json"), old).unwrap();
    assert_eq!(settings::load_in(d.path()).unwrap().brain, Brain::Claude);
    // And a chosen route round-trips by its lowercase name.
    let saved = settings::save_in(d.path(), settings::Settings { brain: Brain::OpenRouter, ..settings::Settings::default() }).unwrap();
    assert!(fs::read_to_string(d.path().join("settings.json")).unwrap().contains("\"brain\": \"openrouter\""));
    assert_eq!(settings::load_in(d.path()).unwrap(), saved);
}

#[test]
fn the_env_per_route() {
    assert!(route_env(Brain::Claude, None).is_empty());
    assert_eq!(
        route_env(Brain::Nvidia, None),
        s(&[("ANTHROPIC_BASE_URL", "http://127.0.0.1:8082"), ("ANTHROPIC_AUTH_TOKEN", "freecc")])
    );
    assert_eq!(route_env(Brain::OpenRouter, Some("ignored")), route_env(Brain::Nvidia, None));
    assert_eq!(
        route_env(Brain::OmniRoute, Some("sk-omni")),
        s(&[
            ("ANTHROPIC_BASE_URL", "http://localhost:20128"),
            ("ANTHROPIC_AUTH_TOKEN", "sk-omni"),
            ("ANTHROPIC_MODEL", "kun-code"),
            ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "kun-fast"),
            ("CLAUDE_CODE_SUBAGENT_MODEL", "kun-fast"),
        ])
    );
}

#[test]
fn merging_strips_other_anthropic_vars_and_the_telemetry_switch() {
    let base = s(&[
        ("TWENTY_API_KEY", "t"),
        ("ANTHROPIC_API_KEY", "sk-ant"),
        ("ANTHROPIC_MODEL", "opus"),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ]);
    for brain in [Brain::Nvidia, Brain::OpenRouter, Brain::OmniRoute] {
        let out = merge_env(base.clone(), route_env(brain, Some("k")));
        assert!(out.contains(&("TWENTY_API_KEY".into(), "t".into())), "{brain:?}");
        assert!(!out.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY" || k == "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"));
        assert_eq!(out.iter().filter(|(k, _)| k == "ANTHROPIC_BASE_URL").count(), 1);
        assert_eq!(out.iter().filter(|(k, _)| k == "ANTHROPIC_MODEL").count(), usize::from(brain == Brain::OmniRoute));
    }
    // Claude leaves the environment exactly as it was.
    assert_eq!(merge_env(base.clone(), route_env(Brain::Claude, None)), base);
}

#[test]
fn builtin_secrets_are_allowed_for_every_profile_and_nothing_else_is() {
    #[derive(Default)]
    struct Mem(RefCell<HashMap<String, String>>);
    impl SecretStore for Mem {
        fn set(&self, a: &str, v: &str) -> Result<(), String> {
            self.0.borrow_mut().insert(a.into(), v.into());
            Ok(())
        }
        fn get(&self, a: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(a).cloned())
        }
    }
    let resolved = profile::resolve_in(Path::new("/nonexistent-kun"), "", crate::driver::DriverKind::Claude).unwrap();
    let store = Mem::default();
    for name in profile::BUILTIN_SECRETS {
        profile::store_secret(&store, &resolved, name, " k\n").unwrap();
        assert!(profile::secret_present(&store, &resolved, name).unwrap());
        assert_eq!(profile::read_secret(&store, &resolved, name).unwrap().as_deref(), Some("k"));
    }
    assert!(matches!(
        profile::store_secret(&store, &resolved, "ANTHROPIC_API_KEY", "k"),
        Err(ProfileError::NotAllowed(_))
    ));
}

fn status(route: Brain, installed: bool, key_set: bool) -> RouteStatus {
    RouteStatus { route, installed, key_set, proxy_up: false, model: route.model().to_owned() }
}

#[test]
fn fallbacks_are_ready_free_routes_and_omniroute_leads_only_a_usable_pool() {
    let all = [
        status(Brain::Claude, true, true),
        status(Brain::OpenRouter, true, true),
        status(Brain::Nvidia, true, true),
        status(Brain::OmniRoute, true, true),
    ];
    assert_eq!(ready_fallbacks(&all, true), vec![Brain::OmniRoute, Brain::Nvidia, Brain::OpenRouter]);
    // Pool not usable: OmniRoute is the last resort.
    assert_eq!(ready_fallbacks(&all, false), vec![Brain::Nvidia, Brain::OpenRouter, Brain::OmniRoute]);
    // Not installed, or no key, is not ready; Claude is never a fallback.
    let some = [
        status(Brain::Claude, true, true),
        status(Brain::OmniRoute, false, true),
        status(Brain::Nvidia, true, false),
        status(Brain::OpenRouter, true, true),
    ];
    assert_eq!(ready_fallbacks(&some, false), vec![Brain::OpenRouter]);
    assert!(ready_fallbacks(&[status(Brain::Claude, true, true)], false).is_empty());
}

#[test]
fn a_usable_pool_is_the_first_fallback_and_needs_only_its_key() {
    // The service kun runs answers, so the omniroute program need not be on the app's path.
    let routes = [
        status(Brain::Claude, true, true),
        status(Brain::OmniRoute, false, true),
        status(Brain::Nvidia, true, true),
        status(Brain::OpenRouter, true, true),
    ];
    assert_eq!(ready_fallbacks(&routes, true), vec![Brain::OmniRoute, Brain::Nvidia, Brain::OpenRouter]);
    // Pool not usable: a missing program still means not ready.
    assert_eq!(ready_fallbacks(&routes, false), vec![Brain::Nvidia, Brain::OpenRouter]);
    // A usable pool without a key is not ready either.
    let keyless = [status(Brain::OmniRoute, true, false), status(Brain::OpenRouter, true, true)];
    assert_eq!(ready_fallbacks(&keyless, true), vec![Brain::OpenRouter]);
    // No duplicates when the pool is both usable and installed.
    let all = [status(Brain::OmniRoute, true, true), status(Brain::Nvidia, true, true)];
    assert_eq!(ready_fallbacks(&all, true), vec![Brain::OmniRoute, Brain::Nvidia]);
}

#[test]
fn a_key_pasted_on_a_brain_card_is_also_written_under_the_bare_name_kun_reads() {
    let resolved = profile::resolve_in(Path::new("/nonexistent-kun"), "", crate::driver::DriverKind::Claude).unwrap();
    let store = Mem::default();
    for name in profile::BUILTIN_SECRETS {
        profile::store_secret(&store, &resolved, name, " k1\n").unwrap();
        assert_eq!(store.get(name).unwrap().as_deref(), Some("k1"), "bare {name}");
        assert_eq!(store.get(&profile::account(&resolved, name)).unwrap().as_deref(), Some("k1"), "scoped {name}");
    }
    // A name outside the built-ins is never written bare, and a rejected one writes nothing.
    assert!(profile::store_secret(&store, &resolved, "ANTHROPIC_API_KEY", "k").is_err());
    assert!(store.get("ANTHROPIC_API_KEY").unwrap().is_none());
    assert!(profile::store_secret(&store, &resolved, "GROQ_API_KEY", "  ").is_err());
}

#[test]
fn the_heartbeat_is_not_ok_only_for_an_installed_gateway_that_is_down() {
    assert!(pool::heartbeat_ok(false, false), "not installed is fine");
    assert!(pool::heartbeat_ok(true, true));
    assert!(!pool::heartbeat_ok(true, false));
    // The light says red or yellow without ever making the machine unhealthy.
    assert!(pool::health(None, None).ok && pool::health(Some(&harvest()), Some(Duration::from_secs(1))).ok);
}

#[test]
fn the_pool_is_usable_only_when_the_ledger_and_the_gateway_agree() {
    let ready = pool::Harvest { pool_ready: true, gateway: pool::Gateway { status: "ok".into() }, ..pool::Harvest::default() };
    assert!(pool::usable(Some(&ready), true));
    assert!(!pool::usable(Some(&ready), false), "the gateway must answer now");
    assert!(!pool::usable(None, true), "no ledger");
    let not_ready = pool::Harvest { pool_ready: false, ..ready.clone() };
    assert!(!pool::usable(Some(&not_ready), true));
    let down = pool::Harvest { gateway: pool::Gateway { status: "down".into() }, ..ready };
    assert!(!pool::usable(Some(&down), true));
}

#[test]
fn fallback_state_is_per_session() {
    assert_eq!(fallback_get("sess-a"), None);
    fallback_set("sess-a", Brain::Nvidia);
    assert_eq!(fallback_get("sess-a"), Some(Brain::Nvidia));
    assert_eq!(fallback_get("sess-b"), None);
}

// ---------------------------------------------------------------- the token pool

const HARVEST: &str = r#"{
  "version": 1, "updated": "2026-11-24T06:42:26.348Z",
  "gateway": {"status": "ok", "url": "http://localhost:20128"}, "pool_ready": true,
  "providers": [
    {"id": "nous", "name": "Nous", "tier": "free", "via": "omniroute", "enabled": true, "status": "connected",
     "connection": "c-1", "requests": 18, "success_pct": 100, "avg_ms": 4284, "quota": null},
    {"id": "cloudflare", "tier": "free-capped", "enabled": true, "status": "exhausted", "connection": "c-2",
     "requests": 37, "success_pct": 89.5, "quota": {"used": "5", "limit": 10, "remaining": 5, "unit": "neurons", "resets_at": 1764000000},
     "note": "no analytics permission"},
    {"id": "groq", "tier": "free", "enabled": true, "status": "needs-key"}
  ],
  "combos": {"kun-code": {"members": ["a/b", "c/d"], "ready": true}, "kun-vision": {"members": [], "ready": false}},
  "totals": {"requests": 55, "tokens": 1, "cost_usd": 0}
}"#;

const CATALOG: &str = r#"{"version": 1, "providers": [
  {"id": "nous", "name": "Nous Portal", "tier": "free", "via": "omniroute", "omniroute": "nous-research", "signup": "https://portal.nousresearch.com", "enabled": true},
  {"id": "cloudflare", "name": "Cloudflare Workers AI", "tier": "free-capped", "via": "omniroute", "omniroute": "cloudflare-ai", "signup": "https://dash.cloudflare.com/profile/api-tokens", "keychain": "CLOUDFLARE_API_TOKEN", "account_keychain": "CLOUDFLARE_ACCOUNT_ID", "needs": ["account_id"], "enabled": true},
  {"id": "groq", "name": "Groq", "tier": "free", "via": "omniroute", "omniroute": "groq", "signup": "https://console.groq.com/keys", "keychain": "GROQ_API_KEY", "enabled": true},
  {"id": "deepseek", "name": "DeepSeek API", "tier": "paid", "via": "omniroute", "omniroute": "deepseek", "keychain": "DEEPSEEK_API_KEY", "decide": "approved, switch on when funded", "enabled": false},
  {"id": "antigravity", "name": "Google Antigravity", "tier": "free", "via": "agy", "login": "Google account in agy", "enabled": true},
  {"id": "claude-max", "name": "Claude Max", "tier": "plan", "via": "claude", "login": "native only", "enabled": true}
]}"#;

fn harvest() -> pool::Harvest {
    serde_json::from_str(HARVEST).unwrap()
}

fn catalog() -> Vec<pool::CatalogProvider> {
    pool::parse_catalog(CATALOG).unwrap()
}

#[test]
fn the_ledger_parses_loosely() {
    let h = harvest();
    let cf = h.providers.iter().find(|r| r.id == "cloudflare").unwrap();
    let q = cf.quota.as_ref().unwrap();
    assert_eq!((q.used, q.limit, q.remaining), (Some(5.0), Some(10.0), Some(5.0)), "a numeric string is a number");
    assert_eq!(q.resets_at.as_deref(), Some("1764000000"));
    assert!(h.providers[0].quota.is_none() && h.combos["kun-code"].ready);
    assert!(pool::parse_catalog("not json").is_err());
}

#[test]
fn pool_health_is_green_yellow_or_red() {
    use pool::PoolLevel::{Green, Red, Yellow};
    let fresh = Some(Duration::from_secs(60));
    let level = |h: Option<&pool::Harvest>, age| pool::health(h, age).level;
    let mut h = harvest();
    // An exhausted enabled free provider: yellow.
    assert_eq!(level(Some(&h), fresh), Yellow);
    assert!(pool::health(Some(&h), fresh).detail.contains("cloudflare"));
    // Nothing wrong and fresh: green.
    h.providers.retain(|r| r.status != "exhausted");
    assert_eq!(level(Some(&h), fresh), Green);
    // Older than two hours, or an unknown age: yellow.
    assert_eq!(level(Some(&h), Some(Duration::from_secs(2 * 3600 + 1))), Yellow);
    assert_eq!(level(Some(&h), None), Yellow);
    // An exhausted paid provider is not the pool's problem.
    h.providers.push(pool::Row { id: "deepseek".into(), tier: "paid".into(), enabled: true, status: "error".into(), ..pool::Row::default() });
    assert_eq!(level(Some(&h), fresh), Green);
    // Gateway not ok, pool not ready, or no ledger at all: red.
    let mut down = h.clone();
    down.gateway.status = "down".into();
    assert_eq!(level(Some(&down), fresh), Red);
    let mut empty = h.clone();
    empty.pool_ready = false;
    assert_eq!(level(Some(&empty), fresh), Red);
    assert_eq!(level(None, None), Red);
}

#[test]
fn the_view_merges_the_catalog_with_the_ledger() {
    use providers::ProviderMode::{Direct, Paid, Plan, Pool};
    let view = providers::build_view(&catalog(), Some(&harvest()), Some(Duration::from_secs(60)), true, true);
    assert_eq!((view.gateway.as_str(), view.pool_ready, view.stale), ("ok", true, false));
    let by = |id: &str| view.rows.iter().find(|r| r.id == id).unwrap().clone();
    // Catalog order is kept.
    assert_eq!(view.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["nous", "cloudflare", "groq", "deepseek", "antigravity", "claude-max"]);
    // Nous has no key to paste; its ledger numbers show.
    let nous = by("nous");
    assert_eq!((nous.mode, nous.requests, nous.success_pct, nous.can_test), (Direct, Some(18.0), Some(100.0), true));
    // Cloudflare takes a key and an account id; its quota and the ledger's note show.
    let cf = by("cloudflare");
    assert_eq!((cf.mode, cf.needs_account, cf.key_set, cf.status.as_str()), (Pool, true, true, "exhausted"));
    assert_eq!(cf.quota.as_ref().and_then(|q| q.remaining), Some(5.0));
    assert_eq!(cf.note.as_deref(), Some("no analytics permission"));
    // Groq has no key yet.
    let groq = by("groq");
    assert_eq!((groq.mode, groq.key_set, groq.can_test), (Pool, false, false));
    // Paid and plan rows are read-only; direct rows carry their sign-in note.
    assert_eq!((by("deepseek").mode, by("deepseek").status.as_str()), (Paid, "off"));
    assert_eq!(by("deepseek").decide.as_deref(), Some("approved, switch on when funded"));
    assert_eq!(by("claude-max").mode, Plan);
    assert_eq!((by("antigravity").mode, by("antigravity").note.as_deref(), by("antigravity").status.as_str()), (Direct, Some("Google account in agy"), "unknown"));
    assert_eq!(view.combos, vec![
        providers::ComboRow { name: "kun-code".into(), ready: true, members: 2 },
        providers::ComboRow { name: "kun-vision".into(), ready: false, members: 0 },
    ]);
    // The gateway is read live: the ledger saying ok does not make it so.
    assert_eq!(providers::build_view(&catalog(), Some(&harvest()), None, false, false).gateway, "down");
    assert!(providers::build_view(&catalog(), Some(&harvest()), None, false, false).stale);
    assert!(!providers::build_view(&catalog(), None, None, false, true).stale);
}

#[derive(Default)]
struct Mem(RefCell<HashMap<String, String>>);
impl SecretStore for Mem {
    fn set(&self, account: &str, value: &str) -> Result<(), String> {
        self.0.borrow_mut().insert(account.to_owned(), value.to_owned());
        Ok(())
    }
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self.0.borrow().get(account).cloned())
    }
}

/// Records every script run and answers from a table keyed by script and first argument.
struct Fake {
    calls: RefCell<Vec<String>>,
    answers: HashMap<&'static str, Result<&'static str, &'static str>>,
}
impl Fake {
    fn new(answers: &[(&'static str, Result<&'static str, &'static str>)]) -> Self {
        Self { calls: RefCell::default(), answers: answers.iter().copied().collect() }
    }
}
impl providers::Runner for Fake {
    fn run(&self, _tool: providers::Tool, script: &str, args: &[&str], _t: Duration) -> Result<String, String> {
        let call = format!("{script} {}", args.join(" ")).trim().to_owned();
        self.calls.borrow_mut().push(call.clone());
        match self.answers.get(call.as_str()) {
            Some(Ok(out)) => Ok((*out).to_owned()),
            Some(Err(e)) => Err((*e).to_owned()),
            None => Ok("{}".to_owned()),
        }
    }
}

#[test]
fn saving_a_key_stores_it_connects_then_rebuilds_and_refreshes_in_order() {
    let store = Mem::default();
    let run = Fake::new(&[("omni.mjs connect cloudflare", Ok("{\"id\":\"cloudflare\",\"status\":\"added\"}"))]);
    providers::save(&store, &run, &catalog(), "cloudflare", " cf-token\n", Some(" acct-1 ")).unwrap();
    // The bare account names kun's scripts read, trimmed.
    assert_eq!(store.get("CLOUDFLARE_API_TOKEN").unwrap().as_deref(), Some("cf-token"));
    assert_eq!(store.get("CLOUDFLARE_ACCOUNT_ID").unwrap().as_deref(), Some("acct-1"));
    assert_eq!(*run.calls.borrow(), ["omni.mjs connect cloudflare", "combos.mjs", "harvest.mjs --quiet"]);
}

#[test]
fn saving_rejects_what_cannot_join_the_pool_and_never_runs_a_script() {
    let store = Mem::default();
    let run = Fake::new(&[]);
    for id in ["deepseek", "claude-max", "antigravity", "nous", "nope"] {
        assert!(providers::save(&store, &run, &catalog(), id, "k", None).is_err(), "{id}");
    }
    assert!(providers::save(&store, &run, &catalog(), "groq", "  ", None).is_err());
    assert!(providers::save(&store, &run, &catalog(), "groq", "bad\u{7}key", None).is_err());
    // An account id for a provider that has no slot for one is refused before anything is stored.
    assert!(providers::save(&store, &run, &catalog(), "groq", "k", Some("a")).is_err());
    assert!(store.0.borrow().is_empty() && run.calls.borrow().is_empty());
}

#[test]
fn a_failed_connect_stops_the_chain_and_reads_as_one_line() {
    let store = Mem::default();
    let timed_out = Fake::new(&[("omni.mjs connect groq", Err("omni.mjs timed out after 60 s"))]);
    let e = providers::save(&store, &timed_out, &catalog(), "groq", "k", None).unwrap_err();
    assert!(e.contains("key is saved") && e.contains("timed out") && !e.contains('\n'), "{e}");
    assert_eq!(*timed_out.calls.borrow(), ["omni.mjs connect groq"]);
    assert!(store.get("GROQ_API_KEY").unwrap().is_some(), "the key stays so a retry is one tap");
    let needs = Fake::new(&[("omni.mjs connect groq", Ok("{\"id\":\"groq\",\"status\":\"needs-key\"}"))]);
    assert!(providers::save(&store, &needs, &catalog(), "groq", "k", None).is_err());
}

#[test]
fn test_reads_the_gateway_connection_and_reports_latency() {
    let h = harvest();
    let ok = Fake::new(&[("omni.mjs test c-2", Ok("{\"valid\":true,\"error\":null,\"latencyMs\":565}"))]);
    let r = providers::test(&ok, Some(&h), "cloudflare").unwrap();
    assert_eq!((r.ok, r.latency_ms, r.message), (true, Some(565.0), None));
    let bad = Fake::new(&[("omni.mjs test c-1", Ok("{\"valid\":false,\"error\":\"401 unauthorized\",\"latencyMs\":90}"))]);
    let r = providers::test(&bad, Some(&h), "nous").unwrap();
    assert_eq!((r.ok, r.message.as_deref()), (false, Some("401 unauthorized")));
    // No connection yet: nothing runs.
    let none = Fake::new(&[]);
    assert!(providers::test(&none, Some(&h), "groq").is_err() && none.calls.borrow().is_empty());
}

#[test]
fn setup_installs_the_service_then_connects_everything_then_refreshes() {
    let run = Fake::new(&[]);
    providers::setup(&run).unwrap();
    assert_eq!(*run.calls.borrow(), ["omniroute.sh install", "omni.mjs setup", "harvest.mjs --quiet"]);
    // A failed install stops there.
    let broken = Fake::new(&[("omniroute.sh install", Err("not installed: npm install -g omniroute"))]);
    assert!(providers::setup(&broken).is_err());
    assert_eq!(broken.calls.borrow().len(), 1);
}

#[test]
fn minting_a_key_starts_a_stopped_gateway_first() {
    let up = Fake::new(&[]);
    providers::mint_key(&up, true).unwrap();
    assert_eq!(*up.calls.borrow(), ["omni.mjs mint-key"]);
    let down = Fake::new(&[]);
    providers::mint_key(&down, false).unwrap();
    assert_eq!(*down.calls.borrow(), ["omniroute.sh install", "omni.mjs mint-key"]);
}

#[test]
fn only_web_addresses_open() {
    assert!(providers::open_url("file:///etc/passwd").is_err());
    assert!(providers::open_url("https://x.test/a b").is_err());
    assert!(providers::open_url("javascript:alert(1)").is_err());
}

#[test]
fn a_key_minted_by_kun_counts_under_its_bare_name() {
    let resolved = profile::resolve_in(Path::new("/nonexistent-kun"), "", crate::driver::DriverKind::Claude).unwrap();
    let store = Mem::default();
    assert!(!key_present(&store, &resolved, "OMNIROUTE_API_KEY"));
    store.set("OMNIROUTE_API_KEY", "sk-minted").unwrap();
    assert!(key_present(&store, &resolved, "OMNIROUTE_API_KEY"));
    assert_eq!(read_key(&store, &resolved, "OMNIROUTE_API_KEY").unwrap().as_deref(), Some("sk-minted"));
    // The profile's own item wins when both exist.
    profile::store_secret(&store, &resolved, "OMNIROUTE_API_KEY", "sk-scoped").unwrap();
    assert_eq!(read_key(&store, &resolved, "OMNIROUTE_API_KEY").unwrap().as_deref(), Some("sk-scoped"));
}

#[test]
fn a_service_started_outside_the_omniroute_folder_is_never_how_the_app_starts_it() {
    let cwd = Proxy::OmniRoute.cwd().unwrap();
    assert!(cwd.ends_with(".omniroute"));
    assert!(Proxy::Fcc.cwd().is_none());
    assert_eq!(Brain::OmniRoute.model(), "kun-code");
}
