//! readiness — what the Fix button does, by the step's `fix.kind`. `auto` is the runner's job
//! (see `run.rs`); the other three are small and live here: Terminal, the opener, a message.

use std::{path::Path, process::Stdio};

use tokio::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

use super::model::{Fix, FixKind, ReadinessError, Step, UninstallKind};

/// What a fix needs done, decided without doing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    /// `provision.mjs --complete --only <id>`.
    Provision(String),
    /// Open Terminal and run this (already prefixed with the cd into kun).
    Terminal(String),
    Link(String),
    /// A message to the owner.
    Request(String),
}

/// Escapes text for an `AppleScript` string literal.
pub(super) fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Single-quotes text for a POSIX shell.
pub(super) fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Whether a command still has a `<PLACEHOLDER>` the person must fill in.
pub(super) fn has_placeholder(cmd: &str) -> bool {
    cmd.find('<')
        .is_some_and(|i| cmd[i..].find('>').is_some_and(|j| j > 1))
}

/// The line Terminal runs for a `terminal` fix: from the kun folder, or, when the command has a
/// placeholder only the person can fill, a note showing what to run and why.
pub(super) fn terminal_line(kun: &Path, fix: &Fix) -> Result<String, ReadinessError> {
    command_line(
        kun,
        fix.cmd.as_deref(),
        fix.message.as_deref(),
        ["the fix has no command", "the fix command has control characters"],
    )
}

/// The line Terminal runs for an uninstall handoff: the catalog's command from the kun folder.
/// Only a `terminal` or keystone uninstall may be handed over; an `auto` one is the runner's.
///
/// # Errors
/// When the step has no uninstall, it is plain `auto`, or its command is empty or has control
/// characters.
pub(super) fn uninstall_line(kun: &Path, step: &Step) -> Result<String, ReadinessError> {
    let un = step
        .uninstall
        .as_ref()
        .filter(|u| u.kind == UninstallKind::Terminal || u.keystone)
        .ok_or_else(|| ReadinessError::NoUninstall(step.id.clone()))?;
    command_line(
        kun,
        Some(&un.cmd),
        None,
        [
            "the uninstall has no command",
            "the uninstall command has control characters",
        ],
    )
}

fn command_line(
    kun: &Path,
    cmd: Option<&str>,
    message: Option<&str>,
    errors: [&'static str; 2],
) -> Result<String, ReadinessError> {
    let cmd = cmd.map(str::trim).filter(|c| !c.is_empty());
    let cmd = cmd.ok_or(ReadinessError::Invalid(errors[0]))?;
    if cmd.chars().any(char::is_control) {
        return Err(ReadinessError::Invalid(errors[1]));
    }
    let dir = shell_quote(&kun.display().to_string());
    if has_placeholder(cmd) {
        let note = message.unwrap_or("Fill in the value, then run:");
        return Ok(format!(
            "cd {dir} && printf '%s\\n' {} {}",
            shell_quote(note),
            shell_quote(cmd)
        ));
    }
    Ok(format!("cd {dir} && {cmd}"))
}

/// The `osascript` arguments that open a Terminal window running `line`.
pub(super) fn osascript_args(line: &str) -> Vec<String> {
    vec![
        "-e".into(),
        format!(
            "tell application \"Terminal\" to do script \"{}\"",
            applescript_escape(line)
        ),
        "-e".into(),
        "tell application \"Terminal\" to activate".into(),
    ]
}

/// A page the person may open: http(s) only, no spaces or control characters.
pub(super) fn valid_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
        && url.len() <= 2048
}

/// Decides what a step's fix does.
///
/// # Errors
/// When the step has no fix, or the fix lacks what its kind needs.
pub(super) fn plan(step: &Step, kun: &Path) -> Result<Action, ReadinessError> {
    let fix = step
        .fix
        .as_ref()
        .ok_or_else(|| ReadinessError::NoFix(step.id.clone()))?;
    match fix.kind {
        FixKind::Auto => Ok(Action::Provision(step.id.clone())),
        FixKind::Terminal => terminal_line(kun, fix).map(Action::Terminal),
        FixKind::Link => match fix.url.as_deref() {
            Some(u) if valid_url(u) => Ok(Action::Link(u.to_owned())),
            _ => Err(ReadinessError::Invalid("the fix has no https link")),
        },
        FixKind::Request => {
            let text = fix
                .message
                .as_deref()
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .ok_or(ReadinessError::Invalid("the fix has no message"))?;
            Ok(Action::Request(format!("[readiness {}] {text}", step.id)))
        }
    }
}

/// A new PowerShell console window running `line`; it stays open.
#[cfg(windows)]
fn open_console(line: &str) -> Result<(), ReadinessError> {
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    std::process::Command::new("powershell.exe")
        .args(["-NoLogo", "-NoExit", "-Command", line])
        .creation_flags(CREATE_NEW_CONSOLE)
        .stdin(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| ReadinessError::Spawn("powershell", e.to_string()))
}

#[cfg(not(windows))]
fn open_console(_line: &str) -> Result<(), ReadinessError> {
    Err(ReadinessError::Unsupported("no console on this platform"))
}

/// Opens Terminal.app running `line`.
///
/// # Errors
/// Off macOS and Windows, or when `osascript` or `PowerShell` fails to start.
pub(super) async fn open_terminal(line: &str) -> Result<(), ReadinessError> {
    if cfg!(windows) {
        return open_console(line);
    }
    if !cfg!(target_os = "macos") {
        return Err(ReadinessError::Unsupported(
            "Terminal fixes need macOS; run the command in your own terminal",
        ));
    }
    let out = Command::new("/usr/bin/osascript")
        .args(osascript_args(line))
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| ReadinessError::Spawn("osascript", e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ReadinessError::Other(
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .chars()
                .take(300)
                .collect(),
        ))
    }
}

/// Opens a page with the system opener.
///
/// # Errors
/// On a URL that is not http(s), or when the opener fails.
pub(super) async fn open_link(url: &str) -> Result<(), ReadinessError> {
    if !valid_url(url) {
        return Err(ReadinessError::Invalid("not an http(s) link"));
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
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| ReadinessError::Spawn("opener", e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(ReadinessError::Other("the opener failed".into()))
    }
}
