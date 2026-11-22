//! settings — who the teammate is, their machine login, department and agent CLI.
//! Stored in `~/.kun/machine/settings.json` (`KUN_HOME` overrides the base for tests).

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
use ts_rs::TS;

use crate::{
    brain::Brain,
    driver::{self, DriverKind, DriverStatus},
    profile::MachineProfile,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export, export_to = "settings/types.ts")]
pub struct Settings {
    pub person: String,
    /// The department chosen in Settings. Empty means the machine's registry default. Saving one
    /// the profile does not authorize is refused; reading one it no longer authorizes gives the default.
    pub department: String,
    /// The repository last chosen in the composer, per department (empty or absent = all repos).
    pub repo_by_department: BTreeMap<String, String>,
    pub driver: DriverKind,
    /// The machine profile to load (`machines/<login>.json`); empty means full access.
    pub login: String,
    /// Opt-in: pick up `run:auto` tasks without being asked. Off by default; nothing spawns yet.
    #[serde(rename = "autoRun")]
    pub auto_run: bool,
    /// Which model runs the Claude Code CLI: `claude` (the person's own sign-in, the default) or a
    /// free route (Settings → Model). Missing in older files, which read as `claude`.
    pub brain: Brain,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            person: String::new(),
            department: String::new(),
            repo_by_department: BTreeMap::new(),
            driver: DriverKind::Claude,
            login: String::new(),
            auto_run: false,
            brain: Brain::Claude,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("name must be at most 80 characters, without control characters")]
    BadPerson,
    #[error("department name is invalid")]
    BadDepartment,
    #[error("this machine is not authorized for that department")]
    Unauthorized,
    #[error("repository name is invalid")]
    BadRepo,
    #[error("login must be lowercase letters, digits and -")]
    BadLogin,
    #[error("could not find the home folder")]
    NoHome,
    #[error("settings.json is damaged ({0}); fix or delete it")]
    Corrupt(String),
    #[error("{0}")]
    Io(#[from] io::Error),
}

impl Serialize for SettingsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl Settings {
    /// Trims the name and checks every field.
    ///
    /// # Errors
    /// When the name is too long or has control characters, or the department or login is unsafe.
    pub fn validated(mut self) -> Result<Self, SettingsError> {
        self.person = self.person.trim().to_owned();
        if self.person.chars().count() > 80 || self.person.chars().any(char::is_control) {
            return Err(SettingsError::BadPerson);
        }
        if !self.department.is_empty() && !crate::pack::valid_name(&self.department) {
            return Err(SettingsError::BadDepartment);
        }
        if self
            .repo_by_department
            .iter()
            // An empty repo is a saved "all repos".
            .any(|(d, r)| !crate::pack::valid_name(d) || (!r.is_empty() && !crate::pack::valid_name(r)))
        {
            return Err(SettingsError::BadRepo);
        }
        self.login = self.login.trim().to_owned();
        if !self.login.is_empty() && !crate::profile::valid_login(&self.login) {
            return Err(SettingsError::BadLogin);
        }
        Ok(self)
    }
}

impl Settings {
    /// Refuses a department the profile does not authorize (empty is the default and always fine).
    ///
    /// # Errors
    /// [`SettingsError::Unauthorized`] when the department is not in the profile's list.
    pub fn authorized_for(&self, profile: &MachineProfile) -> Result<(), SettingsError> {
        if self.department.is_empty()
            || profile.departments.iter().any(|d| d.name == self.department)
        {
            Ok(())
        } else {
            Err(SettingsError::Unauthorized)
        }
    }

    /// Picking the registry's own default stores nothing, so a later change of that default
    /// still reaches this machine.
    #[must_use]
    pub fn normalized(mut self, profile: &MachineProfile) -> Self {
        if self.department == profile.department {
            self.department.clear();
        }
        self
    }

    /// These settings with `department` resolved against the profile: what the app acts on.
    #[must_use]
    pub fn effective(mut self, profile: &MachineProfile) -> Self {
        self.department = crate::profile::effective_department(profile, &self.department);
        self
    }
}

pub(crate) fn machine_dir() -> Result<PathBuf, SettingsError> {
    match std::env::var_os("KUN_HOME") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p).join("machine")),
        _ => dirs::home_dir()
            .map(|h| h.join(".kun/machine"))
            .ok_or(SettingsError::NoHome),
    }
}

/// Defaults when the file does not exist yet.
///
/// # Errors
/// When the file cannot be read or is damaged.
pub fn load_in(dir: &Path) -> Result<Settings, SettingsError> {
    match fs::read_to_string(dir.join("settings.json")) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| SettingsError::Corrupt(e.to_string())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.into()),
    }
}

/// Validates, then writes atomically.
///
/// # Errors
/// On invalid fields or a file failure.
pub fn save_in(dir: &Path, settings: Settings) -> Result<Settings, SettingsError> {
    let settings = settings.validated()?;
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| SettingsError::Corrupt(e.to_string()))?;
    let tmp = dir.join("settings.json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, dir.join("settings.json"))?;
    Ok(settings)
}

/// The saved settings as stored (the department may be empty). See [`load_effective`] for the one to act on.
///
/// # Errors
/// See [`load_in`].
pub fn load() -> Result<Settings, SettingsError> {
    load_in(&machine_dir()?)
}

/// The saved settings with the department resolved against this machine's profile. When the
/// profile cannot be read (kun not cloned yet) the stored value is returned as is.
///
/// # Errors
/// See [`load_in`].
pub fn load_effective() -> Result<Settings, SettingsError> {
    let s = load()?;
    Ok(match crate::profile::current() {
        Ok(r) => s.effective(&r.profile),
        Err(_) => s,
    })
}

/// `load` — the saved settings, or defaults, with the department the machine acts on.
#[tauri::command]
fn load_settings() -> Result<Settings, SettingsError> {
    load_effective()
}

/// `save` — validate, check the department against the profile of the login being saved, store.
/// Without a readable profile (kun not cloned yet) only the name check applies.
#[tauri::command]
fn save(settings: Settings) -> Result<Settings, SettingsError> {
    let mut settings = settings.validated()?;
    if let Ok(r) = crate::pack::kun_dir()
        .map_err(|e| e.to_string())
        .and_then(|k| {
            crate::profile::resolve_in(&k, &settings.login, settings.driver)
                .map_err(|e| e.to_string())
        })
    {
        settings.authorized_for(&r.profile)?;
        settings = settings.normalized(&r.profile);
    }
    save_in(&machine_dir()?, settings)
}

/// `detect` — which agent CLIs are installed here.
#[tauri::command]
fn detect() -> Vec<DriverStatus> {
    driver::detect()
}

#[must_use]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("settings")
        .invoke_handler(tauri::generate_handler![load_settings, save, detect])
        .build()
}

#[cfg(test)]
#[path = "rust_test.rs"]
mod tests;
