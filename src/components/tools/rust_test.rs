use super::*;

#[test]
fn specs_cover_every_requested_tool() {
    let ids: Vec<_> = SPECS.iter().map(|s| s.id).collect();
    for want in ["blender", "ffmpeg", "whisper-cli", "obs", "jev", "comfyui", "artcraft", "filmcraft"] {
        assert!(ids.contains(&want), "{want}");
    }
}

#[test]
fn locate_finds_a_binary_in_a_dir_and_misses_others() {
    let dir = std::env::temp_dir().join(format!("machine-tools-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ffmpeg"), "").unwrap();
    let ffmpeg = SPECS.iter().find(|s| s.id == "ffmpeg").unwrap();
    let jev = SPECS.iter().find(|s| s.id == "jev").unwrap();
    assert_eq!(locate(ffmpeg, std::slice::from_ref(&dir)), Some(dir.join("ffmpeg")));
    assert_eq!(locate(jev, std::slice::from_ref(&dir)), None);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn key_flag_follows_either_env_name() {
    assert!(!key_present(|_| false));
    assert!(key_present(|n| n == "JEV_API_KEY"));
    assert!(key_present(|n| n == "TYPESAFE_API_KEY"));
}

#[test]
fn version_is_the_first_nonempty_line() {
    assert_eq!(first_line("\n ffmpeg version 7.1\nbuilt").as_deref(), Some("ffmpeg version 7.1"));
    assert_eq!(first_line("  \n"), None);
}

#[tokio::test]
async fn version_call_times_out_and_never_hangs() {
    // /bin/sleep treats "5" as seconds; the 2s limit must cut it off.
    let started = std::time::Instant::now();
    assert!(version_of(Path::new("/bin/sleep"), "5").await.is_none());
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[tokio::test]
async fn probe_lists_every_tool_and_only_jev_needs_a_key() {
    let got = probe().await;
    assert_eq!(got.len(), SPECS.len());
    assert!(got.iter().filter(|t| t.id != "jev").all(|t| !t.needs_key));
}
