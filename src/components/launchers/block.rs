//! launchers — the pure part: parse kun's `launchers.json`, render the managed block and splice it
//! into a shell rc file or a `PowerShell` `$PROFILE`.
//!
//! A port of kun's `lib/agents.sh` (`agents_write_alias_block`) and `onboarding-windows.ps1`
//! (`Write-LauncherBlock`): the same markers, the same legacy lines migrated, everything outside
//! the markers kept. Unlike the `PowerShell` original, trailing blank lines before the block are
//! trimmed first, so writing twice gives the same file.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub(super) const BEGIN: &str = "# BEGIN databayt agents";
pub(super) const END: &str = "# END databayt agents";

const BEGIN_UNIX: &str =
    "# BEGIN databayt agents — managed by kun onboarding (lib/agents.sh); edits inside are overwritten";
const BEGIN_WIN: &str =
    "# BEGIN databayt agents — managed by kun onboarding (launchers.json); edits inside are overwritten";
const ENV_LINE: &str = r#"[ -f "$HOME/.claude/.env" ] && set -a && . "$HOME/.claude/.env" && set +a"#;

/// One entry of `<kun>/.claude/machine/launchers.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "launchers/types.ts")]
pub(super) struct Launcher {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub bin: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub bypass_note: Option<String>,
    /// The readiness step that installs the program.
    #[serde(default)]
    pub needs: Option<String>,
}

impl Launcher {
    /// `claude --dangerously-skip-permissions`.
    pub(super) fn command(&self) -> String {
        std::iter::once(self.bin.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Only a plain id is a shell function or alias name.
    pub(super) fn safe(&self) -> bool {
        !self.id.is_empty()
            && self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !self.bin.is_empty()
            && self
                .command()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || " -_./=:".contains(c))
    }
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    launchers: Vec<Launcher>,
}

/// The launchers in catalog order; entries a shell could not safely hold are dropped.
///
/// # Errors
/// When the text is not a launchers file.
pub(super) fn parse(text: &str) -> Result<Vec<Launcher>, String> {
    let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(file.launchers.into_iter().filter(Launcher::safe).collect())
}

/// The shell a block is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shell {
    /// `alias c='…'` in `~/.zshrc` / `~/.bashrc`.
    Posix,
    /// `function c { … $args }` in the `PowerShell` `$PROFILE`.
    PowerShell,
}

fn line_for(l: &Launcher, shell: Shell) -> String {
    match shell {
        Shell::Posix => {
            let line = format!("alias {}='{}'", l.id, l.command());
            let comment = if l.args.is_empty() && l.bypass {
                format!("{} — bypass lives in ~/.config/opencode/opencode.json", l.name)
            } else {
                l.name.clone()
            };
            format!("{line:<52} # {comment}")
        }
        Shell::PowerShell => format!("function {} {{ {} $args }}   # {}", l.id, l.command(), l.name),
    }
}

/// The managed block for the launchers in `enabled` (catalog order), markers included.
pub(super) fn render(launchers: &[Launcher], enabled: &[String], shell: Shell) -> Vec<String> {
    let mut out = vec![match shell {
        Shell::Posix => BEGIN_UNIX.to_owned(),
        Shell::PowerShell => BEGIN_WIN.to_owned(),
    }];
    if shell == Shell::Posix {
        out.push(ENV_LINE.to_owned());
    }
    out.extend(
        launchers
            .iter()
            .filter(|l| enabled.contains(&l.id))
            .map(|l| line_for(l, shell)),
    );
    out.push(END.to_owned());
    out
}

/// Lines the onboarding of old wrote on their own, before the managed block existed.
fn legacy(line: &str, shell: Shell) -> bool {
    match shell {
        Shell::Posix => matches!(
            line,
            "alias c='claude --dangerously-skip-permissions'"
                | "alias a='agy --dangerously-skip-permissions'"
                | "function c { claude --dangerously-skip-permissions \"$@\"; }"
                | "function a { agy --dangerously-skip-permissions \"$@\"; }"
                | "# Claude Code (primary)"
                | "# Antigravity (secondary)"
        ) || line == ENV_LINE,
        Shell::PowerShell => {
            let word = line.strip_prefix("function ").and_then(|r| r.split_once(' '));
            word.is_some_and(|(n, _)| matches!(n, "c" | "a" | "o" | "h" | "cc" | "claw"))
                || [
                    "# Claude Code (primary)",
                    "# Antigravity (secondary)",
                    "# opencode (tertiary",
                    "# Hermes gateway (optional)",
                    "# OpenClaw gateway",
                ]
                .iter()
                .any(|p| line.starts_with(p))
        }
    }
}

/// `text` with its managed block replaced by one for `enabled`: the old block and the legacy lines
/// go, every other line stays, and the new block follows one blank line at the end.
pub(super) fn splice(text: &str, launchers: &[Launcher], enabled: &[String], shell: Shell) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        if line.starts_with(BEGIN) {
            in_block = true;
        } else if line.starts_with(END) {
            in_block = false;
        } else if !in_block && !legacy(line, shell) {
            kept.push(line);
        }
    }
    while kept.last().is_some_and(|l| l.is_empty()) {
        kept.pop();
    }
    let mut out = String::new();
    for l in kept {
        out.push_str(l);
        out.push('\n');
    }
    out.push('\n');
    for l in render(launchers, enabled, shell) {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

/// The command a launcher line defines, if `line` defines launcher `id`.
fn defined(line: &str, id: &str, shell: Shell) -> Option<String> {
    let line = line.trim();
    match shell {
        Shell::Posix => {
            let rest = line.strip_prefix("alias ")?.strip_prefix(id)?.strip_prefix("='")?;
            Some(rest.split('\'').next()?.to_owned())
        }
        Shell::PowerShell => {
            let rest = line.strip_prefix("function ")?.strip_prefix(id)?.trim_start();
            let body = rest.strip_prefix('{')?.split('}').next()?.trim();
            Some(body.strip_suffix("$args").unwrap_or(body).trim().to_owned())
        }
    }
}

/// The ids whose exact command is defined in `text` (inside the block, or as a legacy line).
pub(super) fn enabled_in(text: &str, launchers: &[Launcher], shell: Shell) -> Vec<String> {
    launchers
        .iter()
        .filter(|l| {
            let want = l.command();
            text.lines().any(|line| defined(line, &l.id, shell).is_some_and(|c| c == want))
        })
        .map(|l| l.id.clone())
        .collect()
}

/// `opencode.json` with `"permission": "allow"`: other keys are kept, `$schema` is added when
/// absent. `None` when it already allows. An empty file starts a fresh one.
///
/// # Errors
/// When the text is not a JSON object (it is never overwritten).
pub(super) fn opencode_allow(text: &str) -> Result<Option<String>, String> {
    let mut value: serde_json::Value = if text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(text).map_err(|e| format!("opencode.json is not valid JSON: {e}"))?
    };
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "opencode.json is not a JSON object".to_owned())?;
    if obj.get("permission").and_then(|p| p.as_str()) == Some("allow") {
        return Ok(None);
    }
    obj.entry("$schema")
        .or_insert_with(|| "https://opencode.ai/config.json".into());
    obj.insert("permission".to_owned(), "allow".into());
    let mut out = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    out.push('\n');
    Ok(Some(out))
}
