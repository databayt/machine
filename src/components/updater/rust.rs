//! updater — Machine updates itself from GitHub releases.
//!
//! The endpoint (`latest.json` of the newest release) and the minisign public key live in
//! `tauri.conf.json`; the private key signs releases in CI and never enters the repo. The page
//! checks on launch and every 24 hours, shows the new version, and installs only when the person
//! confirms; then the app restarts.

use serde::Serialize;
use tauri::{
    AppHandle, Runtime,
    plugin::{Builder, TauriPlugin},
};
use tauri_plugin_updater::UpdaterExt;
use ts_rs::TS;

/// A newer version that is ready to install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "updater/types.ts")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdaterError {
    #[error("update check failed: {0}")]
    Check(String),
    #[error("no update to install; check again first")]
    Nothing,
    #[error("install failed: {0}")]
    Install(String),
}

impl Serialize for UpdaterError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// `update_check` — the newer version, or nothing when this one is current.
#[tauri::command]
async fn update_check<R: Runtime>(app: AppHandle<R>) -> Result<Option<UpdateInfo>, UpdaterError> {
    let updater = app
        .updater()
        .map_err(|e| UpdaterError::Check(e.to_string()))?;
    let found = updater
        .check()
        .await
        .map_err(|e| UpdaterError::Check(e.to_string()))?;
    Ok(found.map(|u| UpdateInfo {
        version: u.version,
        current: u.current_version,
        notes: u.body,
    }))
}

/// `update_install` — downloads, verifies the signature, installs and restarts. Called only after
/// the person confirms.
#[tauri::command]
async fn update_install<R: Runtime>(app: AppHandle<R>) -> Result<(), UpdaterError> {
    let updater = app
        .updater()
        .map_err(|e| UpdaterError::Install(e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| UpdaterError::Check(e.to_string()))?
        .ok_or(UpdaterError::Nothing)?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| UpdaterError::Install(e.to_string()))?;
    app.restart()
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("updater")
        .invoke_handler(tauri::generate_handler![update_check, update_install])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
