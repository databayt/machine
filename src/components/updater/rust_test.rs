//! The updater is only as safe as its config: the endpoint, a real public key, and signed artifacts.

fn conf() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../machine/tauri.conf.json")).unwrap()
}

#[test]
fn the_updater_config_is_present_and_points_at_the_release_feed() {
    let c = conf();
    let u = &c["plugins"]["updater"];
    assert_eq!(
        u["endpoints"][0],
        "https://github.com/databayt/machine/releases/latest/download/latest.json"
    );
    let key = u["pubkey"].as_str().unwrap();
    assert!(key.len() > 60, "a minisign public key, not a placeholder");
    assert!(!key.to_ascii_uppercase().contains("PRIVATE"));
    assert_eq!(c["bundle"]["createUpdaterArtifacts"], true);
}

#[test]
fn the_release_workflow_signs_with_secrets_only() {
    let wf = include_str!("../../../.github/workflows/release.yml");
    assert!(wf.contains("TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}"));
    assert!(wf.contains("TAURI_SIGNING_PRIVATE_KEY_PASSWORD"));
    assert!(wf.contains("tags:") && wf.contains("v*") && wf.contains("macos-latest"));
}
