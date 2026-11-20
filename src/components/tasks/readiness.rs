//! The gate. A task may start only when nothing is missing and nothing is in the way:
//! required repos local and not behind, required catalog items installed and current, the
//! profile allowing them, every `after` closed, nobody else holding it, and the WIP limit free.
//! Pure: facts in, verdict out.

use std::collections::BTreeMap;

use super::model::{Context, Readiness, ReadyState, Task, TaskStatus, TasksResponse, TasksView};
use crate::{catalog::ItemStatus, network::model::LocalRepo};

/// What this machine looks like, gathered once per question.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// This machine's kun login; empty when none is set.
    pub me: String,
    pub repos: Vec<LocalRepo>,
    pub items: Vec<(String, ItemStatus)>,
    /// Catalog ids the profile may use; `None` means all.
    pub allowed_items: Option<Vec<String>>,
    /// Repos the profile may use; empty means any.
    pub allowed_repos: Vec<String>,
}

/// `databayt/hogwarts` and `hogwarts` are the same repo.
fn repo_name(s: &str) -> &str {
    s.strip_prefix("databayt/").unwrap_or(s)
}

/// The verdict for one task.
#[must_use]
pub fn readiness(task: &Task, facts: &Facts, in_progress: u32, wip_limit: u32) -> Readiness {
    let mine = !facts.me.is_empty() && task.holder.as_deref() == Some(facts.me.as_str());
    let mut blocked: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    blocked.extend(task.after_open.iter().map(|a| format!("after:{a}")));
    if let Some(holder) = task.holder.as_deref().filter(|_| !mine) {
        blocked.push(format!("held:{holder}"));
    }
    if !mine && in_progress >= wip_limit {
        blocked.push("wip".to_owned());
    }

    for raw in &task.kun.repos {
        let name = repo_name(raw);
        if !facts.allowed_repos.is_empty()
            && !facts.allowed_repos.iter().any(|r| repo_name(r) == name)
        {
            blocked.push(format!("profile:{name}"));
            continue;
        }
        match facts.repos.iter().find(|r| r.name == name) {
            None => missing.push(format!("repo:{name}")),
            Some(r) if r.behind > 0 => missing.push(format!("behind:{name}")),
            Some(_) => {}
        }
    }
    for id in &task.kun.items {
        if facts
            .allowed_items
            .as_ref()
            .is_some_and(|a| !a.contains(id))
        {
            blocked.push(format!("profile:{id}"));
            continue;
        }
        match facts.items.iter().find(|(i, _)| i == id) {
            Some((_, ItemStatus::Installed)) => {}
            _ => missing.push(format!("item:{id}")),
        }
    }

    let state = if !blocked.is_empty() {
        ReadyState::Blocked
    } else if !missing.is_empty() {
        ReadyState::Missing
    } else {
        ReadyState::Ready
    };
    blocked.extend(missing);
    Readiness {
        state,
        missing: blocked,
    }
}

/// The server's response plus a verdict for every task in `mine` and `board`.
#[must_use]
pub fn view(tasks: TasksResponse, facts: &Facts) -> TasksView {
    let mut readiness_map = BTreeMap::new();
    for t in tasks.mine.iter().chain(&tasks.board) {
        readiness_map
            .entry(t.id.clone())
            .or_insert_with(|| readiness(t, facts, tasks.in_progress, tasks.wip_limit));
    }
    TasksView {
        tasks,
        readiness: readiness_map,
    }
}

/// The first auto-run task this machine may pick up: `run:auto`, free, unblocked, still to do, and
/// only when `held` has nothing in progress (one at a time). `board` is in queue order.
#[must_use]
pub fn next_auto_task<'a>(board: &'a [Task], held: &[Task]) -> Option<&'a Task> {
    if held.iter().any(|t| t.status == TaskStatus::InProgress) {
        return None;
    }
    let mut queue: Vec<&Task> = board.iter().collect();
    queue.sort_by_key(|t| t.order);
    queue.into_iter().find(|t| {
        t.auto && t.holder.is_none() && t.status == TaskStatus::Todo && t.after_open.is_empty()
    })
}

/// The seed file's first line: names the task and the repos to open with it.
#[must_use]
pub fn seed_header(task: &Task) -> String {
    let repos: Vec<&str> = task.kun.repos.iter().map(|r| repo_name(r)).collect();
    format!("<!-- kun-task: {} repos={} -->", task.id, repos.join(","))
}

/// Lines of the machine-wide state file that mention one of the task's repos (case-insensitive),
/// at most 40. `None` when nothing matches.
#[must_use]
pub fn aware_excerpt(text: &str, repos: &[String]) -> Option<String> {
    let names: Vec<String> = repos.iter().map(|r| repo_name(r).to_lowercase()).collect();
    if names.is_empty() {
        return None;
    }
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| {
            let lower = l.to_lowercase();
            names.iter().any(|n| lower.contains(n.as_str()))
        })
        .take(40)
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// `TASK.md` with no extra context.
#[must_use]
pub fn seed_text(task: &Task) -> String {
    seed_text_with(task, &Context::default())
}

/// `TASK.md`: the issue, the last comments (receipts marked) and the state excerpt, so a run can
/// start the task without asking.
#[must_use]
pub fn seed_text_with(task: &Task, ctx: &Context) -> String {
    let list = |v: &[String]| {
        if v.is_empty() {
            "none".to_owned()
        } else {
            v.join(", ")
        }
    };
    let mut text = format!(
        "{header}\n# {title}\n\n{id} · {url}\n\nDepartment: {dept} · Sprint: {sprint} · Priority: {prio}\n\n\
         Read KNOWLEDGE.md in this folder first if it exists.\n\n## The issue\n\n{body}\n\n\
         ## Kun block\n\n- Repos (opened with the session): {repos}\n- Items: {items}\n- After: {after}\n",
        header = seed_header(task),
        title = task.title,
        id = task.id,
        url = task.url,
        dept = task.department,
        sprint = task.sprint.as_deref().unwrap_or("none"),
        prio = task
            .priority
            .map_or_else(|| "none".to_owned(), |p| format!("P{p}")),
        body = if task.body.trim().is_empty() {
            "(no description)"
        } else {
            task.body.trim()
        },
        repos = list(&task.kun.repos),
        items = list(&task.kun.items),
        after = list(&task.kun.after),
    );
    if let Some(d) = &task.kun.driver {
        text.push_str(&format!("- Driver: {d}\n"));
    }
    if !ctx.comments.is_empty() {
        text.push_str("\n## Recent comments\n");
        for c in &ctx.comments {
            let mark = if c.receipt { " [run receipt]" } else { "" };
            text.push_str(&format!(
                "\n### {} · {}{mark}\n\n{}\n",
                c.author,
                c.created_at,
                c.body.trim()
            ));
        }
    }
    if let Some(a) = ctx.aware.as_deref().filter(|a| !a.trim().is_empty()) {
        text.push_str(&format!(
            "\n## Machine state (lines about this task's repos)\n\n{}\n",
            a.trim_end()
        ));
    }
    text
}

/// The repos named by a seed's first line, if it is the seed of `id`.
#[must_use]
pub fn seed_repos(text: &str, id: &str) -> Option<Vec<String>> {
    let line = text.lines().next()?;
    let inner = line.strip_prefix("<!-- kun-task: ")?.strip_suffix(" -->")?;
    let (seed_id, repos) = inner.split_once(" repos=")?;
    (seed_id == id).then(|| {
        repos
            .split(',')
            .filter(|r| !r.is_empty())
            .map(str::to_owned)
            .collect()
    })
}
