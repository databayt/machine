//! Starters: the drill-down navigator under the empty home composer. A pack's `starters` is a
//! tree of nodes (branches with `children`, leaves with a `prompt`); a branch may instead name a
//! dynamic `source` (markets, projects, variants) that generates one child per item, each with
//! its own `then` subtree. This file parses and sanitizes the tree, filters it by repo, and
//! produces the dynamic items (cached 60 s). Prompt tokens are filled in the UI.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{PackError, health, kun_dir, valid_name};

const MAX_DEPTH: usize = 8;
const CACHE_TTL: Duration = Duration::from_secs(60);
const LIST_TIMEOUT: Duration = Duration::from_secs(30);

/// Where a branch's children come from when they are generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Markets,
    Projects,
    Variants,
}

/// One row of the navigator: a branch (children / source) or a leaf (prompt).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
#[serde(default)]
pub struct Starter {
    pub id: String,
    pub label_ar: String,
    pub label_en: String,
    pub description_ar: String,
    pub description_en: String,
    /// A lucide icon name from a small fixed set; the UI falls back to a bulb.
    pub icon: Option<String>,
    /// Visible only when the selected repo is listed; empty = any repo.
    pub repos: Vec<String>,
    /// Facets recorded when this node is chosen.
    pub set: BTreeMap<String, String>,
    pub children: Vec<Starter>,
    pub source: Option<SourceKind>,
    /// The subtree under every generated child of `source`.
    pub then: Vec<Starter>,
    /// The leaf's prompt template (`{{repo}}`, `{{facet}}`, `{{facet_label}}`).
    pub prompt: Option<String>,
}

fn clean_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 32 && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn sanitize_one(mut s: Starter, depth: usize) -> Option<Starter> {
    if depth > MAX_DEPTH || s.id.trim().is_empty() {
        return None;
    }
    if s.label_ar.trim().is_empty() {
        s.label_ar.clone_from(&s.label_en);
    }
    if s.label_en.trim().is_empty() {
        s.label_en.clone_from(&s.label_ar);
    }
    if s.label_ar.trim().is_empty() {
        return None;
    }
    s.repos.retain(|r| valid_name(r));
    s.set.retain(|k, _| clean_key(k));
    s.prompt = s.prompt.filter(|p| !p.trim().is_empty());
    s.children = sanitize(s.children, depth + 1);
    s.then = sanitize(s.then, depth + 1);
    // A node that leads nowhere is dropped.
    (s.prompt.is_some() || !s.children.is_empty() || s.source.is_some()).then_some(s)
}

fn sanitize(nodes: Vec<Starter>, depth: usize) -> Vec<Starter> {
    nodes.into_iter().filter_map(|n| sanitize_one(n, depth)).collect()
}

/// Parses the pack's raw `starters` array; a malformed node is skipped, not fatal.
#[must_use]
pub fn parse(raw: &[Value]) -> Vec<Starter> {
    let nodes = raw
        .iter()
        .filter_map(|v| serde_json::from_value::<Starter>(v.clone()).ok())
        .collect();
    sanitize(nodes, 0)
}

/// Keeps what the repo may see: a node with no `repos` is for any repo; a branch whose children
/// are all hidden goes too.
#[must_use]
pub fn filter_repo(nodes: Vec<Starter>, repo: &str) -> Vec<Starter> {
    nodes
        .into_iter()
        .filter(|n| n.repos.is_empty() || n.repos.iter().any(|r| r == repo))
        .filter_map(|mut n| {
            let had_children = !n.children.is_empty();
            let had_then = !n.then.is_empty();
            n.children = filter_repo(n.children, repo);
            n.then = filter_repo(n.then, repo);
            let emptied = (had_children && n.children.is_empty() && n.source.is_none() && n.prompt.is_none())
                || (had_then && n.then.is_empty() && n.prompt.is_none() && n.children.is_empty());
            (!emptied).then_some(n)
        })
        .collect()
}

/// One generated child of a dynamic level.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct SourceItem {
    pub label_ar: String,
    pub label_en: String,
    pub description_ar: String,
    pub description_en: String,
    pub set: BTreeMap<String, String>,
}

/// What a source may depend on.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export, export_to = "pack/types.ts")]
pub struct SourceCtx {
    pub repo: String,
    pub project: Option<String>,
}

fn lang_name(lang: &str) -> (String, String) {
    match lang {
        "ar" => ("عربي".to_owned(), "Arabic".to_owned()),
        "en" => ("إنجليزي".to_owned(), "English".to_owned()),
        other => (other.to_owned(), other.to_owned()),
    }
}

fn market_name(markets: &Value, market: &str) -> (String, String) {
    let name = &markets[market]["name"];
    let en = name["en"].as_str().unwrap_or(market).to_owned();
    let ar = name["ar"].as_str().unwrap_or(&en).to_owned();
    (ar, en)
}

fn variant_item(markets: &Value, market: &str, lang: &str) -> SourceItem {
    let (m_ar, m_en) = market_name(markets, market);
    let (l_ar, l_en) = lang_name(lang);
    let set = BTreeMap::from([
        ("market".to_owned(), market.to_owned()),
        ("lang".to_owned(), lang.to_owned()),
        ("variant".to_owned(), format!("{market}-{lang}")),
    ]);
    SourceItem {
        label_ar: format!("{m_ar} · {l_ar}"),
        label_en: format!("{m_en} · {l_en}"),
        description_ar: String::new(),
        description_en: String::new(),
        set,
    }
}

/// One item per market × language of `markets.json` (the `readme` key is notes, not a market).
#[must_use]
pub fn markets_items(markets_json: &str) -> Vec<SourceItem> {
    let Ok(markets @ Value::Object(_)) = serde_json::from_str::<Value>(markets_json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, m) in markets.as_object().into_iter().flatten() {
        if key == "readme" || !valid_name(key) {
            continue;
        }
        for lang in m["langs"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if valid_name(lang) {
                out.push(variant_item(&markets, key, lang));
            }
        }
    }
    out
}

/// A produce project id: lowercase full names (`^[a-z0-9][a-z0-9-]{2,80}$`) or the legacy
/// uppercase folders (`[A-Z0-9-]+`).
#[must_use]
pub fn valid_project_id(id: &str) -> bool {
    let new = (3..=81).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let legacy = !id.is_empty()
        && id.len() <= 96
        && id.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-');
    new || legacy
}

/// One item per `variants[].id` of a project's `project.json`, named from `markets.json`.
#[must_use]
pub fn variants_items(project_json: &str, markets_json: &str) -> Vec<SourceItem> {
    let (Ok(project), Ok(markets)) = (
        serde_json::from_str::<Value>(project_json),
        serde_json::from_str::<Value>(markets_json),
    ) else {
        return Vec::new();
    };
    project["variants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let id = v["id"].as_str().filter(|i| valid_name(i))?;
            let (m, l) = id.rsplit_once('-').unwrap_or((id, ""));
            let market = v["market"].as_str().unwrap_or(m);
            let lang = v["lang"].as_str().unwrap_or(l);
            let mut item = variant_item(&markets, market, lang);
            item.set.insert("variant".to_owned(), id.to_owned());
            Some(item)
        })
        .collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Project {
    id: String,
    title: String,
    category: String,
    subject: String,
    version: Value,
    next: String,
}

/// `produce.mjs list --json` output (it may follow log noise) as one item per project.
#[must_use]
pub fn projects_items(stdout: &str) -> Vec<SourceItem> {
    let parsed = serde_json::from_str::<Vec<Project>>(stdout.trim()).ok().or_else(|| {
        let start = stdout.find('[')?;
        serde_json::from_str::<Vec<Project>>(stdout[start..].trim()).ok()
    });
    parsed
        .into_iter()
        .flatten()
        .filter(|p| valid_project_id(&p.id))
        .map(|p| {
            let version = match &p.version {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            let head = [p.category.as_str(), version.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" · ");
            let head = if head.is_empty() { p.id.clone() } else { head };
            let (en, ar) = if p.next.is_empty() {
                (head.clone(), head)
            } else {
                (format!("{head} — next: {}", p.next), format!("{head} — التالي: {}", p.next))
            };
            let about = if p.title.is_empty() { p.subject } else { p.title };
            SourceItem {
                label_ar: ar,
                label_en: en,
                description_ar: about.clone(),
                description_en: about,
                set: BTreeMap::from([("project".to_owned(), p.id)]),
            }
        })
        .collect()
}

type Cache = Mutex<HashMap<String, (Instant, Vec<SourceItem>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached(key: &str) -> Option<Vec<SourceItem>> {
    let map = cache().lock().ok()?;
    map.get(key).filter(|(at, _)| at.elapsed() < CACHE_TTL).map(|(_, v)| v.clone())
}

fn remember(key: String, items: &[SourceItem]) {
    if let Ok(mut map) = cache().lock() {
        map.insert(key, (Instant::now(), items.to_vec()));
    }
}

/// `<home>/media/_projects/<id>/project.json`, only when it really lives under `_projects`.
fn project_file(home: &Path, id: &str) -> Option<PathBuf> {
    if !valid_project_id(id) {
        return None;
    }
    let root = home.join("media/_projects").canonicalize().ok()?;
    let file = root.join(id).join("project.json").canonicalize().ok()?;
    file.starts_with(&root).then_some(file)
}

async fn projects_for(repo: &str, home: &Path, kun: &Path) -> Option<Vec<SourceItem>> {
    if !valid_name(repo) {
        return None;
    }
    let argv = ["node", "~/.claude/skills/produce/scripts/produce.mjs", "list", "--json", "--repo", repo]
        .map(str::to_owned);
    let argv = health::safe_argv(&argv, home, kun)
        .inspect_err(|e| tracing::warn!("starter projects: {e}"))
        .ok()?;
    let out = health::run(&argv, LIST_TIMEOUT, home)
        .await
        .inspect_err(|e| tracing::warn!("starter projects: {e}"))
        .ok()?;
    Some(projects_items(&out))
}

/// The items of a dynamic level. Failures give an empty list (logged), never an error.
///
/// # Errors
/// On a bad department name or no home folder.
pub async fn items_for(dept: &str, source: SourceKind, ctx: &SourceCtx) -> Result<Vec<SourceItem>, PackError> {
    if !valid_name(dept) {
        return Err(PackError::BadName);
    }
    let kun = kun_dir()?;
    let home = dirs::home_dir().ok_or(PackError::NoHome)?;
    let key = format!("{dept}|{source:?}|{}|{}", ctx.repo, ctx.project.as_deref().unwrap_or(""));
    if let Some(hit) = cached(&key) {
        return Ok(hit);
    }
    let markets = || fs::read_to_string(kun.join("content/media/markets.json")).unwrap_or_default();
    let items = match source {
        SourceKind::Markets => markets_items(&markets()),
        SourceKind::Variants => ctx
            .project
            .as_deref()
            .and_then(|id| project_file(&home, id))
            .and_then(|f| fs::read_to_string(f).ok())
            .map(|p| variants_items(&p, &markets()))
            .unwrap_or_default(),
        SourceKind::Projects => match projects_for(&ctx.repo, &home, &kun).await {
            Some(items) => items,
            None => return Ok(Vec::new()),
        },
    };
    remember(key, &items);
    Ok(items)
}

#[cfg(test)]
#[path = "starters_test.rs"]
mod tests;
