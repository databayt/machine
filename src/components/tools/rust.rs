//! tools — a read-only probe of the production tools installed on this machine.
//!
//! Nothing is launched except `<tool> --version`, with no shell and a 2s limit. The jev API
//! key is never read: only whether one of its env names is set at all.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::Serialize;
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::process::Command;
use ts_rs::TS;

/// One production tool and whether this machine has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "tools/types.ts")]
pub struct ToolStatus {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// jev only: true until `JEV_API_KEY` or `TYPESAFE_API_KEY` is set in the environment.
    pub needs_key: bool,
}

struct Spec {
    id: &'static str,
    label: &'static str,
    /// Binary names looked up in the search dirs.
    bins: &'static [&'static str],
    /// App bundles that count as installed even without a CLI.
    apps: &'static [&'static str],
    /// Argument that prints a version, if the tool has one.
    version_arg: Option<&'static str>,
}

const SPECS: [Spec; 8] = [
    Spec { id: "blender", label: "Blender", bins: &["blender"], apps: &["/Applications/Blender.app/Contents/MacOS/Blender"], version_arg: Some("--version") },
    Spec { id: "ffmpeg", label: "FFmpeg", bins: &["ffmpeg"], apps: &[], version_arg: Some("-version") },
    Spec { id: "whisper-cli", label: "Whisper", bins: &["whisper-cli"], apps: &[], version_arg: Some("--version") },
    Spec { id: "obs", label: "OBS Studio", bins: &["obs"], apps: &["/Applications/OBS.app/Contents/MacOS/OBS"], version_arg: None },
    Spec { id: "jev", label: "Jev", bins: &["jev"], apps: &[], version_arg: Some("--version") },
    Spec { id: "comfyui", label: "ComfyUI", bins: &["comfyui", "comfy"], apps: &["/Applications/ComfyUI.app/Contents/MacOS/ComfyUI"], version_arg: None },
    Spec { id: "artcraft", label: "ArtCraft", bins: &["artcraft"], apps: &["/Applications/ArtCraft.app/Contents/MacOS/ArtCraft"], version_arg: None },
    Spec { id: "filmcraft", label: "FilmCraft", bins: &["filmcraft"], apps: &["/Applications/FilmCraft.app/Contents/MacOS/FilmCraft"], version_arg: None },
];

const KEY_ENVS: [&str; 2] = ["JEV_API_KEY", "TYPESAFE_API_KEY"];
const VERSION_TIMEOUT: Duration = Duration::from_secs(2);

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = dirs::home_dir()
        .map(|h| vec![h.join(".local/bin")])
        .unwrap_or_default();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

fn locate(spec: &Spec, dirs: &[PathBuf]) -> Option<PathBuf> {
    spec.bins
        .iter()
        .find_map(|b| dirs.iter().map(|d| d.join(b)).find(|p| p.is_file()))
        .or_else(|| spec.apps.iter().map(PathBuf::from).find(|p| p.is_file()))
}

/// Where a tool's program is, by its id (`blender`, `ffmpeg`, `whisper-cli`); `None` when not installed.
#[must_use]
pub fn find(id: &str) -> Option<PathBuf> {
    let spec = SPECS.iter().find(|s| s.id == id)?;
    locate(spec, &search_dirs())
}

/// The key is only tested for presence, never read into the output.
fn key_present(is_set: impl Fn(&str) -> bool) -> bool {
    KEY_ENVS.iter().any(|n| is_set(n))
}

/// First non-empty line of version output, capped.
fn first_line(out: &str) -> Option<String> {
    let line = out.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.chars().take(120).collect())
}

async fn version_of(path: &Path, arg: &str) -> Option<String> {
    let child = Command::new(path)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let out = tokio::time::timeout(VERSION_TIMEOUT, child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    first_line(&String::from_utf8_lossy(&out.stdout))
}

/// Probes every tool. Read-only.
pub async fn probe() -> Vec<ToolStatus> {
    let dirs = search_dirs();
    let needs_key = !key_present(|n| std::env::var_os(n).is_some_and(|v| !v.is_empty()));
    let mut out = Vec::with_capacity(SPECS.len());
    for spec in &SPECS {
        let path = locate(spec, &dirs);
        let version = match (&path, spec.version_arg) {
            (Some(p), Some(arg)) => version_of(p, arg).await,
            _ => None,
        };
        out.push(ToolStatus {
            id: spec.id.into(),
            label: spec.label.into(),
            installed: path.is_some(),
            path: path.map(|p| p.display().to_string()),
            version,
            needs_key: spec.id == "jev" && needs_key,
        });
    }
    out
}

/// `tools` — which production tools this machine has.
#[tauri::command]
async fn tools() -> Vec<ToolStatus> {
    probe().await
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tools")
        .invoke_handler(tauri::generate_handler![tools])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
