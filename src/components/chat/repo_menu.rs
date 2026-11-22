//! The repository menu: the repos a run may be limited to, and what the menu shows for each.
//!
//! The list is the union of the selected department's pack `repos` and the profile's repos,
//! limited to the repos kun's registry (`.claude/memory/repositories.json`) marks `active`, with
//! hogwarts first when present. A repo that has a `word` there shows it (the marketing site is
//! `landing`); its name, the folder and the GitHub repo, never changes. One function feeds both the
//! menu (`repo_menu`) and the check behind every run (`validate_repo`).

use std::{fs, path::Path};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::{pack, profile::MachineProfile};

/// The repo that leads the menu and is the default.
pub const FIRST: &str = "hogwarts";

/// One row of the menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "chat/types.ts")]
pub struct RepoEntry {
    /// The repo's name: its GitHub name and its folder.
    pub name: String,
    /// The word shown instead of the name when the registry has one.
    pub word: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    id: String,
    active: bool,
    word: Option<String>,
}

/// The repos of the registry file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    rows: Vec<Row>,
}

impl Registry {
    fn get(&self, name: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.id == name)
    }
}

/// Reads the registry's text: `repositories` is an object of groups (arrays) or one array; every
/// object with a string `id` is a repo. `None` when no repo is found (a damaged or empty file).
#[must_use]
pub fn parse_registry(text: &str) -> Option<Registry> {
    let v: Value = serde_json::from_str(text).ok()?;
    let groups: Vec<&Value> = match v.get("repositories")? {
        Value::Object(o) => o.values().collect(),
        a @ Value::Array(_) => vec![a],
        _ => return None,
    };
    let rows: Vec<Row> = groups
        .into_iter()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|r| {
            let id = r.get("id")?.as_str()?.to_owned();
            let word = r
                .get("word")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|w| pack::valid_name(w))
                .map(str::to_owned);
            Some(Row { id, active: r.get("status").and_then(Value::as_str) == Some("active"), word })
        })
        .collect();
    (!rows.is_empty()).then_some(Registry { rows })
}

/// `<kun>/.claude/memory/repositories.json`; `None` when it is missing or unreadable.
#[must_use]
pub fn read_registry(kun: &Path) -> Option<Registry> {
    parse_registry(&fs::read_to_string(kun.join(".claude/memory/repositories.json")).ok()?)
}

/// The profile's repos for a department: its own list, else the ones its departments tag.
#[must_use]
pub fn profile_repos(profile: &MachineProfile, department: &str) -> Vec<String> {
    if profile.repos.is_empty() {
        profile
            .departments
            .iter()
            .filter(|d| d.name == department)
            .flat_map(|d| d.repos.iter().map(|r| r.name.clone()))
            .collect()
    } else {
        profile.repos.clone()
    }
}

/// The menu: pack repos then profile repos, deduplicated and safe to name, limited to active registry
/// repos (all of them when there is no registry), hogwarts first.
#[must_use]
pub fn menu(registry: Option<&Registry>, pack_repos: &[String], profile_repos: &[String]) -> Vec<RepoEntry> {
    let mut names: Vec<&String> = Vec::new();
    for r in pack_repos.iter().chain(profile_repos) {
        if pack::valid_name(r) && !names.contains(&r) {
            names.push(r);
        }
    }
    if let Some(reg) = registry {
        names.retain(|n| reg.get(n).is_some_and(|row| row.active));
    }
    if let Some(i) = names.iter().position(|n| n.as_str() == FIRST) {
        let first = names.remove(i);
        names.insert(0, first);
    }
    names
        .into_iter()
        .map(|n| RepoEntry {
            name: n.clone(),
            word: registry.and_then(|reg| reg.get(n)).and_then(|row| row.word.clone()),
        })
        .collect()
}

/// The menu for one department of this machine, read from kun's checkout.
///
/// # Errors
/// When the department's name is invalid.
pub fn menu_for(kun: &Path, profile: &MachineProfile, department: &str) -> Result<Vec<RepoEntry>, pack::PackError> {
    let meta = pack::meta_in(kun, department)?;
    Ok(menu(read_registry(kun).as_ref(), &meta.repos, &profile_repos(profile, department)))
}
