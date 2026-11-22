use super::block::{self, Launcher, Shell};

fn set() -> Vec<Launcher> {
    block::parse(
        r#"{"version":1,"launchers":[
        {"id":"c","name":"Claude Code (primary)","bin":"claude","args":["--dangerously-skip-permissions"],"bypass":true,"needs":"claude.cli"},
        {"id":"a","name":"Antigravity (secondary)","bin":"agy","args":["--dangerously-skip-permissions"],"bypass":true},
        {"id":"o","name":"opencode (tertiary)","bin":"opencode","args":[],"bypass":true,"bypassNote":"config"},
        {"id":"h","name":"Hermes gateway (optional)","bin":"hermes","args":[],"bypass":false},
        {"id":"x; rm","name":"bad","bin":"x"},
        {"id":"y","name":"bad","bin":"x","args":["$(id)"]}]}"#,
    )
    .unwrap()
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_catalog_parses_and_drops_what_a_shell_could_not_hold() {
    let l = set();
    assert_eq!(l.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["c", "a", "o", "h"]);
    assert_eq!(l[0].command(), "claude --dangerously-skip-permissions");
    assert_eq!(l[2].command(), "opencode");
    assert!(block::parse("not json").is_err());
}

#[test]
fn the_posix_block_matches_agents_sh() {
    let b = block::render(&set(), &ids(&["c", "o"]), Shell::Posix);
    assert_eq!(b.len(), 5);
    assert!(b[0].starts_with("# BEGIN databayt agents — managed by kun onboarding (lib/agents.sh)"));
    assert_eq!(b[1], r#"[ -f "$HOME/.claude/.env" ] && set -a && . "$HOME/.claude/.env" && set +a"#);
    assert_eq!(
        b[2],
        format!("{:<52} # Claude Code (primary)", "alias c='claude --dangerously-skip-permissions'")
    );
    assert_eq!(
        b[3],
        format!("{:<52} # opencode (tertiary) — bypass lives in ~/.config/opencode/opencode.json", "alias o='opencode'")
    );
    assert_eq!(b[4], "# END databayt agents");
}

#[test]
fn the_windows_block_is_functions_with_args_and_no_env_line() {
    let b = block::render(&set(), &ids(&["c", "h"]), Shell::PowerShell);
    assert_eq!(
        b,
        [
            "# BEGIN databayt agents — managed by kun onboarding (launchers.json); edits inside are overwritten",
            "function c { claude --dangerously-skip-permissions $args }   # Claude Code (primary)",
            "function h { hermes $args }   # Hermes gateway (optional)",
            "# END databayt agents",
        ]
    );
}

#[test]
fn splicing_twice_is_the_same_file_and_outside_lines_survive() {
    let rc = "export PATH=\"$HOME/bin:$PATH\"\nalias gs='git status'\n\n# my note\n";
    let once = block::splice(rc, &set(), &ids(&["c", "a"]), Shell::Posix);
    assert!(once.starts_with("export PATH=\"$HOME/bin:$PATH\"\nalias gs='git status'\n\n# my note\n\n# BEGIN databayt agents"));
    assert!(once.ends_with("# END databayt agents\n"));
    let twice = block::splice(&once, &set(), &ids(&["c", "a"]), Shell::Posix);
    assert_eq!(once, twice);
}

#[test]
fn toggling_rewrites_only_the_block() {
    let on = block::splice("alias gs='git status'\n", &set(), &ids(&["c", "a", "o", "h"]), Shell::Posix);
    assert_eq!(block::enabled_in(&on, &set(), Shell::Posix), ["c", "a", "o", "h"]);
    let off = block::splice(&on, &set(), &ids(&["c", "o", "h"]), Shell::Posix);
    assert_eq!(block::enabled_in(&off, &set(), Shell::Posix), ["c", "o", "h"]);
    assert!(off.starts_with("alias gs='git status'\n\n# BEGIN"));
    assert!(!off.contains("alias a="));
    let none = block::splice(&off, &set(), &[], Shell::Posix);
    assert!(block::enabled_in(&none, &set(), Shell::Posix).is_empty());
    assert!(none.contains("# END databayt agents"));
}

#[test]
fn legacy_lines_move_into_the_block() {
    let legacy = "alias c='claude --dangerously-skip-permissions'\n# Claude Code (primary)\n\
                  function a { agy --dangerously-skip-permissions \"$@\"; }\n\
                  [ -f \"$HOME/.claude/.env\" ] && set -a && . \"$HOME/.claude/.env\" && set +a\n\
                  export KEEP=1\n";
    assert_eq!(block::enabled_in(legacy, &set(), Shell::Posix), ["c"]);
    let out = block::splice(legacy, &set(), &ids(&["c"]), Shell::Posix);
    assert_eq!(out.matches("alias c=").count(), 1);
    assert_eq!(out.matches("set -a").count(), 1);
    assert!(out.starts_with("export KEEP=1\n\n# BEGIN"));
    assert!(!out.contains("function a"));
}

#[test]
fn a_different_command_is_not_the_launcher() {
    let rc = "alias c='claude'\nalias a='agy --dangerously-skip-permissions'\n";
    assert_eq!(block::enabled_in(rc, &set(), Shell::Posix), ["a"]);
}

#[test]
fn the_windows_profile_round_trips_and_migrates_legacy_functions() {
    let profile = "Set-Alias ll ls\nfunction c { claude --dangerously-skip-permissions @args }\nfunction cc { claude }\n# Claude Code (primary)\n";
    let out = block::splice(profile, &set(), &ids(&["c", "a"]), Shell::PowerShell);
    assert!(out.starts_with("Set-Alias ll ls\n\n# BEGIN databayt agents"));
    assert_eq!(out.matches("function c ").count(), 1);
    assert!(!out.contains("function cc"));
    assert_eq!(block::enabled_in(&out, &set(), Shell::PowerShell), ["c", "a"]);
    assert_eq!(out, block::splice(&out, &set(), &ids(&["c", "a"]), Shell::PowerShell));
}

#[test]
fn opencode_gets_permission_allow_without_losing_keys() {
    let out = block::opencode_allow(r#"{"theme":"dark","permission":{"bash":"ask"}}"#).unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["permission"], "allow");
    assert_eq!(v["theme"], "dark");
    assert_eq!(v["$schema"], "https://opencode.ai/config.json");
    assert!(out.ends_with('\n'));
    assert_eq!(block::opencode_allow(&out).unwrap(), None);
    assert!(block::opencode_allow("").unwrap().unwrap().contains("allow"));
    assert!(block::opencode_allow("{nope").is_err());
    assert!(block::opencode_allow("[1]").is_err());
}

mod disk {
    use super::super::*;
    use super::{ids, set};

    #[test]
    fn apply_writes_every_file_backs_up_once_and_keeps_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let zsh = home.path().join(".zshrc");
        let bash = home.path().join(".bashrc");
        fs::write(&zsh, "export A=1\nalias c='claude --dangerously-skip-permissions'\n").unwrap();
        fs::write(&bash, "export B=2\n").unwrap();
        let files = targets(home.path(), Shell::Posix, None);
        assert_eq!(files, [zsh.clone(), bash.clone()]);

        apply_with(&set(), &ids(&["c", "o"]), &files, Shell::Posix, home.path()).unwrap();
        let z = fs::read_to_string(&zsh).unwrap();
        assert!(z.starts_with("export A=1\n\n# BEGIN databayt agents"));
        assert_eq!(z.matches("alias c=").count(), 1);
        // the first change leaves the pristine original, later ones keep it
        let backup = home.path().join(".zshrc.kun-backup");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "export A=1\nalias c='claude --dangerously-skip-permissions'\n");
        apply_with(&set(), &ids(&["c"]), &files, Shell::Posix, home.path()).unwrap();
        assert_eq!(fs::read_to_string(&backup).unwrap().lines().count(), 2);
        // nothing changed, nothing rewritten
        let before = fs::read_to_string(&bash).unwrap();
        apply_with(&set(), &ids(&["c"]), &files, Shell::Posix, home.path()).unwrap();
        assert_eq!(before, fs::read_to_string(&bash).unwrap());

        let view = status_with(&set(), &files, |b| b == "claude");
        let row = |id: &str| view.rows.iter().find(|r| r.id == id).unwrap().clone();
        assert!(row("c").enabled && row("c").installed);
        assert!(!row("a").enabled && !row("a").installed);
        assert_eq!(row("c").command, "claude --dangerously-skip-permissions");
    }

    #[test]
    fn opencode_is_allowed_only_when_its_launcher_is_on() {
        let home = tempfile::tempdir().unwrap();
        let files = targets(home.path(), Shell::Posix, None);
        apply_with(&set(), &ids(&["c"]), &files, Shell::Posix, home.path()).unwrap();
        let cfg = home.path().join(".config/opencode/opencode.json");
        assert!(!cfg.exists());
        apply_with(&set(), &ids(&["c", "o"]), &files, Shell::Posix, home.path()).unwrap();
        assert!(fs::read_to_string(&cfg).unwrap().contains("\"permission\": \"allow\""));
    }

    #[test]
    fn an_unknown_id_and_a_damaged_opencode_file_are_refused() {
        let home = tempfile::tempdir().unwrap();
        let files = targets(home.path(), Shell::Posix, None);
        assert!(matches!(
            apply_with(&set(), &ids(&["zz"]), &files, Shell::Posix, home.path()),
            Err(LauncherError::Unknown(_))
        ));
        assert!(!files[0].exists());
        let cfg = home.path().join(".config/opencode/opencode.json");
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        fs::write(&cfg, "{broken").unwrap();
        assert!(apply_with(&set(), &ids(&["o"]), &files, Shell::Posix, home.path()).is_err());
        assert_eq!(fs::read_to_string(&cfg).unwrap(), "{broken");
    }

    #[test]
    fn the_windows_profile_goes_under_documents() {
        let home = tempfile::tempdir().unwrap();
        let files = targets(home.path(), Shell::PowerShell, None);
        assert!(files[0].ends_with("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"));
        apply_with(&set(), &ids(&["c"]), &files, Shell::PowerShell, home.path()).unwrap();
        let text = fs::read_to_string(&files[0]).unwrap();
        assert!(text.contains("function c { claude --dangerously-skip-permissions $args }"));
    }
}
