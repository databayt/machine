use std::{collections::BTreeSet, fs, path::Path};

use super::model::ConfigError;
use super::{
    guard::{self, Backup, Verdict},
    mcp, settings,
};

const KUN_SETTINGS: &str = include_str!("fixtures/kun-settings.json");
const MCP_JSON: &str = include_str!("fixtures/mcp.json");
const MCP_LIST: &str = include_str!("fixtures/mcp-list.txt");

/// A `claude` folder and a `kun` checkout in temp dirs, with the user's settings.
fn world(user_settings: &str) -> (tempfile::TempDir, tempfile::TempDir) {
    let claude = tempfile::tempdir().unwrap();
    let kun = tempfile::tempdir().unwrap();
    fs::create_dir_all(kun.path().join(".claude")).unwrap();
    fs::write(kun.path().join(".claude/settings.json"), KUN_SETTINGS).unwrap();
    fs::write(kun.path().join(".claude/mcp.json"), MCP_JSON).unwrap();
    fs::write(claude.path().join("settings.json"), user_settings).unwrap();
    (claude, kun)
}

fn managed() -> BTreeSet<String> {
    ["$schema", "fallbackModel", "env", "permissions", "hooks"]
        .map(String::from)
        .into()
}

#[test]
fn only_model_voice_and_keys_outside_kuns_template_are_editable() {
    let m = managed();
    assert!(settings::editable("model", Some(&m)));
    assert!(settings::editable("voice", Some(&m)));
    assert!(settings::editable("remoteControlAtStartup", Some(&m)));
    assert!(
        !settings::editable("fallbackModel", Some(&m)),
        "kun manages it"
    );
    assert!(!settings::editable("hooks", Some(&m)));
    // Even a key the template does not mention cannot run commands or grant permissions.
    assert!(!settings::editable("apiKeyHelper", Some(&m)));
    assert!(!settings::editable("statusLine", Some(&m)));
    // Without the template the app fails closed: model and voice only.
    assert!(settings::editable("model", None));
    assert!(!settings::editable("remoteControlAtStartup", None));
}

#[test]
fn the_view_lists_editable_keys_and_hides_secret_looking_ones() {
    let (claude, kun) = world(
        r#"{"model":"opus","fallbackModel":["x"],"env":{"A":"1"},"someToken":"abc","theme":"dark"}"#,
    );
    fs::write(
        claude.path().join(".kun-notify"),
        "morning digest\nsecond line\n",
    )
    .unwrap();
    let view = super::load_view(claude.path(), kun.path()).unwrap();
    let keys: Vec<&str> = view.settings.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["model", "someToken", "theme"]);
    let token = view.settings.iter().find(|s| s.key == "someToken").unwrap();
    assert!(
        token.hidden && token.value.is_empty(),
        "a secret-looking value is never shown"
    );
    assert_eq!(view.notify.as_deref(), Some("morning digest"));
    assert!(view.template_found && !view.env_present);
    assert!(view.managed.contains(&"hooks".to_owned()));
}

#[test]
fn env_is_only_present_or_absent_never_read() {
    let (claude, kun) = world("{}");
    assert!(
        !super::load_view(claude.path(), kun.path())
            .unwrap()
            .env_present
    );
    fs::write(claude.path().join(".env"), "SECRET=hunter2\n").unwrap();
    let view = super::load_view(claude.path(), kun.path()).unwrap();
    assert!(view.env_present);
    assert!(!serde_json::to_string(&view).unwrap().contains("hunter2"));
}

#[test]
fn an_edit_backs_up_validates_and_writes_atomically() {
    let (claude, kun) = world(r#"{"model":"opus","theme":"dark"}"#);
    let backup = settings::edit(
        claude.path(),
        kun.path(),
        "model",
        Some("\"sonnet\""),
        1_700_000_000,
    )
    .unwrap();
    let saved = backup.saved.clone().unwrap();
    assert_eq!(
        saved,
        claude.path().join("state/backups/settings.json.1700000000")
    );
    assert!(fs::read_to_string(&saved).unwrap().contains("opus"));
    let now: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(claude.path().join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(now["model"], "sonnet");
    assert_eq!(now["theme"], "dark", "the rest of the file is untouched");
    assert!(
        fs::read_dir(claude.path()).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".machine-tmp")),
        "no temp file is left behind"
    );
}

#[test]
fn a_bad_edit_changes_nothing() {
    let (claude, kun) = world(r#"{"model":"opus"}"#);
    let before = fs::read_to_string(claude.path().join("settings.json")).unwrap();
    for (key, value) in [
        ("model", Some("{not json")),
        ("hooks", Some("{}")),
        ("fallbackModel", Some("[]")),
        ("bad key", Some("1")),
        ("someToken", Some("\"x\"")),
    ] {
        assert!(
            settings::edit(claude.path(), kun.path(), key, value, 1).is_err(),
            "{key}"
        );
    }
    assert_eq!(
        fs::read_to_string(claude.path().join("settings.json")).unwrap(),
        before
    );
    assert!(
        !claude.path().join("state/backups").exists(),
        "a refused edit makes no backup"
    );
}

#[test]
fn removing_a_key_works_and_is_guarded_the_same_way() {
    let (claude, kun) = world(r#"{"model":"opus","theme":"dark"}"#);
    settings::edit(claude.path(), kun.path(), "theme", None, 5).unwrap();
    let text = fs::read_to_string(claude.path().join("settings.json")).unwrap();
    assert!(!text.contains("theme") && text.contains("model"));
}

#[test]
fn a_red_verdict_restores_the_backup() {
    let (claude, kun) = world(r#"{"model":"opus"}"#);
    let original = fs::read_to_string(claude.path().join("settings.json")).unwrap();
    let backup = settings::edit(claude.path(), kun.path(), "model", Some("\"sonnet\""), 9).unwrap();
    assert!(
        fs::read_to_string(claude.path().join("settings.json"))
            .unwrap()
            .contains("sonnet")
    );
    let out = guard::finish(&backup, Verdict::Green, Verdict::Red, Backup::restore).unwrap();
    assert!(!out.ok && out.rolled_back && out.verdict == "red");
    assert_eq!(
        fs::read_to_string(claude.path().join("settings.json")).unwrap(),
        original
    );
}

#[test]
fn a_green_or_unknown_verdict_keeps_the_change_and_an_already_red_one_is_not_blamed() {
    let (claude, kun) = world(r#"{"model":"opus"}"#);
    let backup = settings::edit(claude.path(), kun.path(), "model", Some("\"sonnet\""), 9).unwrap();
    for (before, after) in [
        (Verdict::Green, Verdict::Green),
        (Verdict::Green, Verdict::Warn),
        (Verdict::Green, Verdict::Unknown),
        (Verdict::Red, Verdict::Red),
    ] {
        let out = guard::finish(&backup, before, after, |_| panic!("must not restore")).unwrap();
        assert!(out.ok && !out.rolled_back);
    }
}

#[test]
fn restoring_a_file_that_did_not_exist_removes_it() {
    let (claude, _kun) = world("{}");
    let line = settings::set_notify(claude.path(), "hello", 3).unwrap();
    assert!(claude.path().join(".kun-notify").exists());
    assert!(line.saved.is_none());
    line.restore().unwrap();
    assert!(!claude.path().join(".kun-notify").exists());
}

#[test]
fn restore_key_puts_back_only_the_servers() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(".claude.json");
    fs::write(&target, r#"{"mcpServers":{"a":{}},"projects":{"p":1}}"#).unwrap();
    let backup = guard::backup(dir.path(), &target, "claude.json", 7).unwrap();
    // A running session changed other keys and the call removed a server.
    fs::write(
        &target,
        r#"{"mcpServers":{},"projects":{"p":2},"newKey":true}"#,
    )
    .unwrap();
    backup.restore_key("mcpServers").unwrap();
    let now: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(now["mcpServers"]["a"], serde_json::json!({}));
    assert_eq!(now["projects"]["p"], 2, "the sessions' changes stay");
    assert_eq!(now["newKey"], true);
}

#[test]
fn backups_are_pruned_to_twenty() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("settings.json");
    fs::write(&target, "{}").unwrap();
    for i in 0..25 {
        guard::backup(dir.path(), &target, "settings.json", 100 + i).unwrap();
    }
    let n = fs::read_dir(dir.path().join("state/backups"))
        .unwrap()
        .count();
    assert_eq!(n, 20);
    assert!(dir.path().join("state/backups/settings.json.124").exists());
    assert!(!dir.path().join("state/backups/settings.json.100").exists());
    assert!(matches!(
        guard::backup(dir.path(), &target, "../x", 1),
        Err(ConfigError::BadName(_))
    ));
}

#[test]
fn health_json_is_read_for_its_verdict() {
    assert_eq!(
        guard::parse_verdict("noise\n{\"verdict\":\"red\",\"errors\":2}\n"),
        Verdict::Red
    );
    assert_eq!(
        guard::parse_verdict("{\"verdict\":\"warn\"}"),
        Verdict::Warn
    );
    assert_eq!(
        guard::parse_verdict("{\"verdict\":\"green\"}"),
        Verdict::Green
    );
    assert_eq!(guard::parse_verdict("not json"), Verdict::Unknown);
    assert_eq!(
        guard::parse_verdict("{\"verdict\":\"purple\"}"),
        Verdict::Unknown
    );
}

#[test]
fn notify_is_one_short_line() {
    assert_eq!(settings::valid_notify("  hi  ").unwrap(), "hi");
    for bad in ["", "   ", "two\nlines", &"x".repeat(201)] {
        assert!(settings::valid_notify(bad).is_err());
    }
}

#[test]
fn mcp_list_parses_and_redacts() {
    let servers = mcp::parse_list(MCP_LIST);
    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["context7", "github", "airtable"]);
    assert_eq!(servers[0].connected, Some(true));
    assert_eq!(servers[1].connected, None);
    assert_eq!(servers[2].connected, Some(false));
    assert!(
        !servers[2].summary.contains("patABCDEF"),
        "{}",
        servers[2].summary
    );
}

#[test]
fn mcp_calls_are_vectors_and_the_catalog_entry_loses_its_notes() {
    assert_eq!(
        mcp::remove_args("context7").unwrap(),
        ["mcp", "remove", "context7", "-s", "user"]
    );
    assert!(mcp::remove_args("--scope").is_err());
    let (_c, kun) = world("{}");
    let args = mcp::add_args(kun.path(), "airtable").unwrap();
    assert_eq!(&args[..5], ["mcp", "add-json", "-s", "user", "airtable"]);
    let json: serde_json::Value = serde_json::from_str(&args[5]).unwrap();
    assert!(json.get("description").is_none() && json.get("$skipRegistration").is_none());
    assert_eq!(json["command"], "npx");
    assert_eq!(
        json["env"]["AIRTABLE_API_KEY"], "${AIRTABLE_API_KEY}",
        "a placeholder, never a value"
    );
    assert!(matches!(
        mcp::add_args(kun.path(), "nope"),
        Err(ConfigError::NotInCatalog(_))
    ));
}

#[test]
fn the_catalog_offers_what_is_not_installed() {
    let (_c, kun) = world("{}");
    let installed = mcp::parse_list("context7: npx - ✓ Connected\n");
    let view = mcp::view(kun.path(), installed);
    assert_eq!(view.available.len(), 1);
    assert_eq!(view.available[0].name, "airtable");
    assert!(view.available[0].skipped);
}

#[test]
fn the_secrets_action_takes_a_gist_id_and_nothing_else() {
    let line = super::secrets_line(Path::new("/Users/x/kun"), "a1b2c3d4e5f6").unwrap();
    assert_eq!(
        line,
        "cd '/Users/x/kun' && bash .claude/scripts/secrets.sh a1b2c3d4e5f6"
    );
    for bad in [
        "",
        "abc",
        "a1b2c3; rm -rf ~",
        "$(id)abcdef",
        "a b c d e f g",
    ] {
        assert!(
            super::secrets_line(Path::new("/k"), bad).is_err(),
            "{bad:?}"
        );
    }
}
