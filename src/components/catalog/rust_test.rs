use std::{fs, path::Path};

use super::*;

/// A throwaway kun checkout + claude dir + kun home, with a registry built from real files.
struct World {
    _root: tempfile::TempDir,
    env: Env,
}

fn item(id: &str, deps: &[&str], files: &[(&str, &str, &str)]) -> serde_json::Value {
    let (kind, name) = id.split_once(':').unwrap();
    serde_json::json!({
        "id": id, "name": name, "type": format!("kun:{kind}"), "title": name,
        "description": "", "keywords": [], "dependencies": deps, "version": format!("v-{id}"),
        "files": files.iter().map(|(p, t, body)| serde_json::json!({
            "path": p, "target": t, "sha256": sha256_hex(body.as_bytes()),
        })).collect::<Vec<_>>(),
    })
}

fn world(items: &[serde_json::Value], sources: &[(&str, &str)]) -> World {
    let root = tempfile::tempdir().unwrap();
    let env = Env {
        kun_dir: root.path().join("kun"),
        claude_dir: root.path().join("claude"),
        kun_home: root.path().join("home"),
    };
    for (path, body) in sources {
        let p = env.kun_dir.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }
    let reg = env.kun_dir.join("public/r/kun/index.json");
    fs::create_dir_all(reg.parent().unwrap()).unwrap();
    fs::write(reg, serde_json::json!({ "items": items }).to_string()).unwrap();
    World { _root: root, env }
}

/// funnel skill (2 files), lead agent, and a pack that needs both.
fn standard() -> World {
    world(
        &[
            item(
                "skill:funnel",
                &[],
                &[
                    (
                        ".claude/skills/funnel/SKILL.md",
                        "skills/funnel/SKILL.md",
                        "# funnel",
                    ),
                    (
                        ".claude/skills/funnel/scripts/run.sh",
                        "skills/funnel/scripts/run.sh",
                        "echo hi",
                    ),
                ],
            ),
            item(
                "agent:lead",
                &[],
                &[(".claude/agents/lead.md", "agents/lead.md", "# lead")],
            ),
            item(
                "pack:sales",
                &["skill:funnel", "agent:lead"],
                &[(
                    "departments/sales/pack.json",
                    "departments/sales/pack.json",
                    "{}",
                )],
            ),
        ],
        &[
            (".claude/skills/funnel/SKILL.md", "# funnel"),
            (".claude/skills/funnel/scripts/run.sh", "echo hi"),
            (".claude/agents/lead.md", "# lead"),
            ("departments/sales/pack.json", "{}"),
        ],
    )
}

fn status(env: &Env, id: &str) -> ItemStatus {
    list_items(env)
        .unwrap()
        .into_iter()
        .find(|i| i.id == id)
        .unwrap()
        .status
}

fn rel_files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    v.sort();
    v
}

#[test]
fn fixture_registry_parses() {
    let reg: Registry = serde_json::from_str(include_str!("fixtures/index.json")).unwrap();
    assert_eq!(reg.items.len(), 3);
    let pack = reg.items.iter().find(|i| i.id == "pack:sales").unwrap();
    assert_eq!(pack.kind, "kun:pack");
    assert!(pack.dependencies.contains(&"agent:lead".to_string()));
    assert_eq!(pack.status, ItemStatus::Available);
}

#[test]
fn install_copies_files_and_records_lock() {
    let w = standard();
    assert_eq!(status(&w.env, "skill:funnel"), ItemStatus::Available);
    assert_eq!(
        install_item(&w.env, "skill:funnel").unwrap(),
        vec!["skill:funnel"]
    );
    assert_eq!(
        fs::read_to_string(w.env.claude_dir.join("skills/funnel/SKILL.md")).unwrap(),
        "# funnel"
    );
    assert_eq!(status(&w.env, "skill:funnel"), ItemStatus::Installed);
    let lock = read_lock(&w.env);
    assert_eq!(lock.items["skill:funnel"].files.len(), 2);
    // No temp files left behind.
    assert!(
        rel_files(&w.env.claude_dir)
            .iter()
            .all(|f| !f.contains("kun-tmp"))
    );
}

#[test]
fn pack_installs_dependencies_first() {
    let w = standard();
    let done = install_item(&w.env, "pack:sales").unwrap();
    assert_eq!(done, vec!["skill:funnel", "agent:lead", "pack:sales"]);
    assert!(w.env.claude_dir.join("agents/lead.md").is_file());
    assert!(
        w.env
            .claude_dir
            .join("departments/sales/pack.json")
            .is_file()
    );
}

#[test]
fn edited_target_is_outdated_and_reinstall_fixes_it() {
    let w = standard();
    install_item(&w.env, "agent:lead").unwrap();
    fs::write(w.env.claude_dir.join("agents/lead.md"), "# edited").unwrap();
    assert_eq!(status(&w.env, "agent:lead"), ItemStatus::Outdated);
    install_item(&w.env, "agent:lead").unwrap();
    assert_eq!(status(&w.env, "agent:lead"), ItemStatus::Installed);
}

#[test]
fn new_registry_version_is_outdated() {
    let w = standard();
    install_item(&w.env, "agent:lead").unwrap();
    // The checkout moves on: new source content, new hash.
    fs::write(w.env.kun_dir.join(".claude/agents/lead.md"), "# lead v2").unwrap();
    let reg = item(
        "agent:lead",
        &[],
        &[(".claude/agents/lead.md", "agents/lead.md", "# lead v2")],
    );
    fs::write(
        w.env.registry_path(),
        serde_json::json!({ "items": [reg] }).to_string(),
    )
    .unwrap();
    assert_eq!(status(&w.env, "agent:lead"), ItemStatus::Outdated);
    install_item(&w.env, "agent:lead").unwrap();
    assert_eq!(
        fs::read_to_string(w.env.claude_dir.join("agents/lead.md")).unwrap(),
        "# lead v2"
    );
}

#[test]
fn uninstall_removes_only_recorded_files() {
    let w = standard();
    install_item(&w.env, "skill:funnel").unwrap();
    let mine = w.env.claude_dir.join("skills/funnel/notes.md");
    fs::write(&mine, "mine").unwrap();
    let other = w.env.claude_dir.join("skills/other/SKILL.md");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, "other").unwrap();

    uninstall_item(&w.env, "skill:funnel").unwrap();
    assert!(!w.env.claude_dir.join("skills/funnel/SKILL.md").exists());
    assert!(
        !w.env.claude_dir.join("skills/funnel/scripts").exists(),
        "empty dir is pruned"
    );
    assert!(mine.is_file(), "a file the user made survives");
    assert!(other.is_file());
    assert!(read_lock(&w.env).items.is_empty());
    assert_eq!(status(&w.env, "skill:funnel"), ItemStatus::Available);
}

#[test]
fn uninstall_refuses_while_a_dependent_is_installed() {
    let w = standard();
    install_item(&w.env, "pack:sales").unwrap();
    let err = uninstall_item(&w.env, "skill:funnel").unwrap_err();
    assert!(matches!(err, CatalogError::InUse(_, ref by) if by == "pack:sales"));
    assert!(w.env.claude_dir.join("skills/funnel/SKILL.md").is_file());
    uninstall_item(&w.env, "pack:sales").unwrap();
    uninstall_item(&w.env, "skill:funnel").unwrap();
}

#[test]
fn target_escaping_claude_dir_is_refused() {
    for bad in ["../evil.md", "/tmp/evil.md", "skills/../../evil.md"] {
        let w = world(
            &[item(
                "skill:evil",
                &[],
                &[(".claude/skills/evil/SKILL.md", bad, "x")],
            )],
            &[(".claude/skills/evil/SKILL.md", "x")],
        );
        let err = install_item(&w.env, "skill:evil").unwrap_err();
        assert!(matches!(err, CatalogError::Unsafe(_)), "{bad}: {err}");
        assert!(!w.env.claude_dir.parent().unwrap().join("evil.md").exists());
    }
}

#[cfg(unix)]
#[test]
fn symlinked_directory_cannot_smuggle_a_write_out() {
    let w = standard();
    let outside = w.env.claude_dir.parent().unwrap().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(&w.env.claude_dir).unwrap();
    std::os::unix::fs::symlink(&outside, w.env.claude_dir.join("agents")).unwrap();
    let err = install_item(&w.env, "agent:lead").unwrap_err();
    assert!(matches!(err, CatalogError::Unsafe(_)));
    assert!(rel_files(&outside).is_empty());
}

#[test]
fn source_that_no_longer_matches_the_registry_is_refused() {
    let w = standard();
    fs::write(w.env.kun_dir.join(".claude/agents/lead.md"), "tampered").unwrap();
    let err = install_item(&w.env, "agent:lead").unwrap_err();
    assert!(matches!(err, CatalogError::Stale(_)));
    assert!(!w.env.claude_dir.join("agents/lead.md").exists());
}

#[test]
fn names_are_validated() {
    let w = standard();
    for bad in [
        "",
        "../etc",
        "Skill:funnel",
        "skill:fun nel",
        "skill/funnel",
        "skill:funnel\n",
    ] {
        assert!(
            matches!(install_item(&w.env, bad), Err(CatalogError::InvalidName)),
            "{bad:?}"
        );
        assert!(
            matches!(uninstall_item(&w.env, bad), Err(CatalogError::InvalidName)),
            "{bad:?}"
        );
    }
    assert!(matches!(
        install_item(&w.env, "skill:nope"),
        Err(CatalogError::UnknownItem(_))
    ));
}

#[test]
fn missing_registry_says_how_to_build_it() {
    let w = world(&[], &[]);
    fs::remove_file(w.env.registry_path()).unwrap();
    let msg = list_items(&w.env).unwrap_err().to_string();
    assert!(msg.contains("pnpm kun:registry"), "{msg}");
}

#[cfg(unix)]
#[test]
fn executable_bit_is_kept() {
    use std::os::unix::fs::PermissionsExt;
    let w = standard();
    let src = w.env.kun_dir.join(".claude/skills/funnel/scripts/run.sh");
    fs::set_permissions(&src, fs::Permissions::from_mode(0o755)).unwrap();
    install_item(&w.env, "skill:funnel").unwrap();
    let mode = fs::metadata(w.env.claude_dir.join("skills/funnel/scripts/run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111);
}

// ---- profile scope ----

fn profile_for(depts: &[&str], catalog: CatalogAccess) -> MachineProfile {
    use crate::{
        driver::DriverKind,
        profile::{Access, Department},
    };
    MachineProfile {
        department: String::new(),
        login: "moed".into(),
        name: String::new(),
        os: "windows".into(),
        driver: DriverKind::Claude,
        departments: depts
            .iter()
            .map(|d| Department {
                name: (*d).into(),
                title: (*d).into(),
                title_ar: None,
                quick_actions: Vec::new(),
                skills: Vec::new(),
                cards: Vec::new(),
                repos: Vec::new(),
            })
            .collect(),
        repos: Vec::new(),
        access: Access {
            catalog,
            ..Access::FULL
        },
        hidden: Vec::new(),
    }
}

/// `standard()` plus a second pack and a skill that only it needs.
fn two_packs() -> World {
    let w = standard();
    let reg = w.env.kun_dir.join("public/r/kun/index.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&reg).unwrap()).unwrap();
    let items = json["items"].as_array_mut().unwrap();
    items.push(item(
        "skill:edit",
        &[],
        &[(
            ".claude/skills/edit/SKILL.md",
            "skills/edit/SKILL.md",
            "# edit",
        )],
    ));
    items.push(item(
        "pack:video",
        &["skill:edit"],
        &[(
            "departments/video/pack.json",
            "departments/video/pack.json",
            "{}",
        )],
    ));
    fs::write(&reg, json.to_string()).unwrap();
    for (p, body) in [
        (".claude/skills/edit/SKILL.md", "# edit"),
        ("departments/video/pack.json", "{}"),
    ] {
        let path = w.env.kun_dir.join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    w
}

#[test]
fn a_profile_lists_only_its_packs_and_what_they_need() {
    let w = two_packs();
    let profile = profile_for(&["sales"], CatalogAccess::Profile);
    let allowed = allowed_ids(&w.env, &profile).unwrap();
    let ids: Vec<String> = list_items_scoped(&w.env, allowed.as_ref())
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, vec!["skill:funnel", "agent:lead", "pack:sales"]);
    assert_eq!(
        list_items(&w.env).unwrap().len(),
        5,
        "unscoped still lists everything"
    );
}

#[test]
fn full_catalog_access_means_no_limit() {
    let w = two_packs();
    assert!(
        allowed_ids(&w.env, &profile_for(&["sales"], CatalogAccess::All))
            .unwrap()
            .is_none()
    );
}

#[test]
fn install_and_uninstall_refuse_outside_the_profile() {
    let w = two_packs();
    let allowed = allowed_ids(&w.env, &profile_for(&["sales"], CatalogAccess::Profile)).unwrap();
    for outside in ["pack:video", "skill:edit"] {
        assert!(
            matches!(
                install_item_scoped(&w.env, outside, allowed.as_ref()),
                Err(CatalogError::OutsideProfile(_))
            ),
            "{outside}"
        );
        assert!(
            matches!(
                uninstall_item_scoped(&w.env, outside, allowed.as_ref()),
                Err(CatalogError::OutsideProfile(_))
            ),
            "{outside}"
        );
    }
    assert!(
        !w.env.claude_dir.join("skills/edit").exists(),
        "nothing was written"
    );

    let done = install_item_scoped(&w.env, "pack:sales", allowed.as_ref()).unwrap();
    assert_eq!(done.last().map(String::as_str), Some("pack:sales"));
    uninstall_item_scoped(&w.env, "pack:sales", allowed.as_ref()).unwrap();
}

#[test]
fn hidden_lanes_never_install_even_through_their_pack() {
    let w = two_packs();
    let mut profile = profile_for(&["video"], CatalogAccess::Profile);
    profile.hidden = vec!["skill:edit".into()];
    let allowed = allowed_ids(&w.env, &profile).unwrap();
    let ids: Vec<String> = list_items_scoped(&w.env, allowed.as_ref())
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, vec!["pack:video"], "the hidden skill is not listed");
    assert!(matches!(
        install_item_scoped(&w.env, "skill:edit", allowed.as_ref()),
        Err(CatalogError::OutsideProfile(_))
    ));
    let done = install_item_scoped(&w.env, "pack:video", allowed.as_ref()).unwrap();
    assert_eq!(
        done,
        vec!["pack:video"],
        "the pack installs without its hidden lane"
    );
    assert!(!w.env.claude_dir.join("skills/edit").exists());
}

// ── the real tree, enable / disable ───────────────────────────────────────────────────────────

/// Every kind in one registry: funnel (2 files), lead, a rule, a hook, a pack, a skill that is
/// never on disk, and a `machine` item the tree must ignore.
fn tree_world() -> World {
    world(
        &[
            item(
                "skill:funnel",
                &[],
                &[
                    (
                        ".claude/skills/funnel/SKILL.md",
                        "skills/funnel/SKILL.md",
                        "# funnel",
                    ),
                    (
                        ".claude/skills/funnel/scripts/run.sh",
                        "skills/funnel/scripts/run.sh",
                        "echo hi",
                    ),
                ],
            ),
            item(
                "agent:lead",
                &[],
                &[(".claude/agents/lead.md", "agents/lead.md", "# lead")],
            ),
            item(
                "rule:style",
                &[],
                &[(".claude/rules/style.md", "rules/style.md", "# style")],
            ),
            item(
                "hook:guard",
                &[],
                &[(".claude/hooks/guard.sh", "hooks/guard.sh", "echo guard")],
            ),
            item(
                "pack:sales",
                &[],
                &[(
                    "departments/sales/pack.json",
                    "departments/sales/pack.json",
                    "{}",
                )],
            ),
            item(
                "skill:gone",
                &[],
                &[(
                    ".claude/skills/gone/SKILL.md",
                    "skills/gone/SKILL.md",
                    "# gone",
                )],
            ),
            item(
                "machine:app",
                &[],
                &[("machine/app.json", "machine/app.json", "{}")],
            ),
        ],
        &[
            (".claude/skills/funnel/SKILL.md", "# funnel"),
            (".claude/skills/funnel/scripts/run.sh", "echo hi"),
            (".claude/agents/lead.md", "# lead"),
            (".claude/rules/style.md", "# style"),
            (".claude/hooks/guard.sh", "echo guard"),
            ("departments/sales/pack.json", "{}"),
            (".claude/skills/gone/SKILL.md", "# gone"),
            ("machine/app.json", "{}"),
        ],
    )
}

/// Writes a file under `~/.claude` directly, the way the engine's own installer would.
fn put(env: &Env, rel: &str, body: &str) {
    let p = env.claude_dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

fn find<'a>(entries: &'a [Entry], id: &str) -> &'a Entry {
    entries
        .iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("no entry {id}"))
}

fn state_of(env: &Env, id: &str) -> EntryState {
    find(&tree_entries(env).unwrap(), id).state
}

fn disabled_json(env: &Env) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(env.disabled_path()).unwrap()).unwrap()
}

#[test]
fn the_scan_finds_every_kind_and_classifies_it() {
    use EntryState::{Available, Installed, Outdated, Personal};
    use Owner::{Kun, You};
    let w = tree_world();
    let env = &w.env;
    put(env, "skills/funnel/SKILL.md", "# funnel");
    put(env, "skills/funnel/scripts/run.sh", "echo hi");
    put(env, "agents/lead.md", "# lead but edited"); // a hash differs
    put(env, "rules/style.md", "# style"); // shipped by the engine, no lock entry
    // Things a person made.
    put(env, "skills/mine/SKILL.md", "# mine");
    put(env, "skills/mine/notes/a.md", "a");
    put(env, "skills/mine/.hidden", "x");
    put(env, "skills/not-a-skill/readme.md", "no SKILL.md");
    put(env, "agents/mine.md", "# mine");
    put(env, "agents/.dot.md", "hidden");
    put(env, "rules/mine.md", "# mine");
    put(env, "hooks/mine.sh", "echo");
    put(env, "hooks/notes.txt", "not a hook");
    put(env, "departments/mine/pack.json", "{}");
    #[cfg(unix)]
    {
        let outside = env.claude_dir.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("SKILL.md"), "x").unwrap();
        std::os::unix::fs::symlink(&outside, env.claude_dir.join("skills/leaky")).unwrap();
    }

    let tree = tree_entries(env).unwrap();
    let ids: Vec<&str> = tree.iter().map(|e| e.id.as_str()).collect();
    // Sorted by folder, then name; no `machine` item, no dot-file, no link leaving ~/.claude.
    assert_eq!(
        ids,
        [
            "agent:lead",
            "agent:mine",
            "pack:mine",
            "pack:sales",
            "hook:guard",
            "hook:mine",
            "rule:mine",
            "rule:style",
            "skill:funnel",
            "skill:gone",
            "skill:mine",
        ]
    );
    let flags = |id: &str| {
        let e = find(&tree, id);
        (
            e.state,
            e.owner,
            e.can_install,
            e.can_uninstall,
            e.can_toggle,
        )
    };
    assert_eq!(flags("skill:funnel"), (Installed, Kun, false, true, true));
    assert_eq!(flags("agent:lead"), (Outdated, Kun, true, true, true));
    assert_eq!(flags("rule:style"), (Installed, Kun, false, true, true));
    assert_eq!(flags("hook:guard"), (Available, Kun, true, false, false));
    assert_eq!(flags("pack:sales"), (Available, Kun, true, false, false));
    assert_eq!(flags("skill:gone"), (Available, Kun, true, false, false));
    assert_eq!(flags("skill:mine"), (Personal, You, false, false, true));
    assert_eq!(flags("agent:mine"), (Personal, You, false, false, true));
    assert_eq!(flags("rule:mine"), (Personal, You, false, false, true));
    assert_eq!(flags("hook:mine"), (Personal, You, false, false, false));
    assert_eq!(flags("pack:mine"), (Personal, You, false, false, false));

    let funnel = find(&tree, "skill:funnel");
    assert_eq!(funnel.dir, "skills");
    assert_eq!(funnel.version, "v-skill:funnel");
    assert_eq!(
        funnel.files,
        ["skills/funnel/SKILL.md", "skills/funnel/scripts/run.sh"]
    );
    let mine = find(&tree, "skill:mine");
    assert_eq!(
        (mine.title.as_str(), mine.description.as_str()),
        ("mine", "")
    );
    assert_eq!(mine.version, "");
    assert_eq!(
        mine.files,
        ["skills/mine/SKILL.md", "skills/mine/notes/a.md"]
    );
    assert_eq!(find(&tree, "hook:mine").files, ["hooks/mine.sh"]);
}

#[test]
fn a_missing_registry_still_shows_what_is_on_disk() {
    let w = world(&[], &[]);
    fs::remove_file(w.env.registry_path()).unwrap();
    put(&w.env, "agents/mine.md", "# mine");
    let tree = tree_entries(&w.env).unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].state, EntryState::Personal);
}

#[test]
fn the_profile_limits_kuns_units_but_never_a_persons_own() {
    let w = tree_world();
    put(&w.env, "skills/mine/SKILL.md", "# mine");
    put(&w.env, "agents/lead.md", "# lead");
    let allowed: BTreeSet<String> = ["skill:funnel".to_string()].into();
    let tree = tree_entries_scoped(&w.env, Some(&allowed)).unwrap();
    let ids: Vec<&str> = tree.iter().map(|e| e.id.as_str()).collect();
    // `agent:lead` is on disk and in the registry but outside the profile: not "personal" either.
    assert_eq!(ids, ["skill:funnel", "skill:mine"]);
    assert!(matches!(
        disable_item_scoped(&w.env, "agent:lead", Some(&allowed)),
        Err(CatalogError::OutsideProfile(_))
    ));
    disable_item_scoped(&w.env, "skill:mine", Some(&BTreeSet::new())).unwrap();
}

#[test]
fn disable_moves_the_unit_aside_and_enable_restores_its_bytes() {
    let w = tree_world();
    let env = &w.env;
    install_item(env, "skill:funnel").unwrap();
    assert_eq!(state_of(env, "skill:funnel"), EntryState::Installed);

    disable_item(env, "skill:funnel").unwrap();
    assert!(!env.claude_dir.join("skills/funnel").exists());
    assert_eq!(
        fs::read_to_string(env.store_dir().join("skills/funnel/scripts/run.sh")).unwrap(),
        "echo hi"
    );
    assert_eq!(state_of(env, "skill:funnel"), EntryState::Disabled);
    let entry = find(&tree_entries(env).unwrap(), "skill:funnel").clone();
    assert!(entry.can_toggle && entry.can_uninstall && !entry.can_install);
    // The lock entry is left alone.
    assert!(read_lock(env).items.contains_key("skill:funnel"));

    // The record's shape is `setup.sh`'s contract.
    let json = disabled_json(env);
    assert_eq!(json.as_object().unwrap().len(), 2);
    assert_eq!(json["removed"], serde_json::json!([]));
    let held = &json["disabled"]["skill:funnel"];
    assert_eq!(held["files"], serde_json::json!(["skills/funnel"]));
    assert_eq!(held.as_object().unwrap().len(), 2);
    assert!(
        held["at"]
            .as_str()
            .is_some_and(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())),
        "{held}"
    );
    // Moved aside twice is refused.
    assert!(matches!(
        disable_item(env, "skill:funnel"),
        Err(CatalogError::Conflict(_))
    ));

    enable_item(env, "skill:funnel").unwrap();
    assert_eq!(
        fs::read_to_string(env.claude_dir.join("skills/funnel/SKILL.md")).unwrap(),
        "# funnel"
    );
    assert_eq!(
        fs::read_to_string(env.claude_dir.join("skills/funnel/scripts/run.sh")).unwrap(),
        "echo hi"
    );
    assert!(!env.store_dir().join("skills/funnel").exists());
    assert_eq!(disabled_json(env)["disabled"], serde_json::json!({}));
    assert_eq!(state_of(env, "skill:funnel"), EntryState::Installed);
}

#[test]
fn an_agent_file_toggles_too() {
    let w = tree_world();
    put(&w.env, "agents/lead.md", "# lead");
    disable_item(&w.env, "agent:lead").unwrap();
    assert!(w.env.store_dir().join("agents/lead.md").is_file());
    assert!(!w.env.claude_dir.join("agents/lead.md").exists());
    enable_item(&w.env, "agent:lead").unwrap();
    assert_eq!(
        fs::read_to_string(w.env.claude_dir.join("agents/lead.md")).unwrap(),
        "# lead"
    );
}

#[test]
fn hooks_and_packs_cannot_be_toggled() {
    let w = tree_world();
    put(&w.env, "hooks/guard.sh", "echo guard");
    put(&w.env, "hooks/mine.sh", "echo");
    put(&w.env, "departments/sales/pack.json", "{}");
    for id in ["hook:guard", "hook:mine", "pack:sales"] {
        assert!(
            matches!(
                disable_item(&w.env, id),
                Err(CatalogError::NotToggleable(_))
            ),
            "{id}"
        );
        assert!(
            matches!(enable_item(&w.env, id), Err(CatalogError::NotToggleable(_))),
            "{id}"
        );
        assert!(!find(&tree_entries(&w.env).unwrap(), id).can_toggle);
    }
    assert!(w.env.claude_dir.join("hooks/guard.sh").is_file());
    // Something that is not on disk or held has nothing to toggle.
    assert!(matches!(
        disable_item(&w.env, "skill:gone"),
        Err(CatalogError::UnknownItem(_))
    ));
    assert!(matches!(
        enable_item(&w.env, "skill:funnel"),
        Err(CatalogError::UnknownItem(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_symlinked_unit_is_refused() {
    let w = tree_world();
    let env = &w.env;
    put(env, "skills/real/SKILL.md", "# real");
    std::os::unix::fs::symlink(
        env.claude_dir.join("skills/real"),
        env.claude_dir.join("skills/alias"),
    )
    .unwrap();
    let outside = env.claude_dir.parent().unwrap().join("outside.md");
    fs::write(&outside, "x").unwrap();
    fs::create_dir_all(env.claude_dir.join("agents")).unwrap();
    std::os::unix::fs::symlink(&outside, env.claude_dir.join("agents/out.md")).unwrap();
    for id in ["skill:alias", "agent:out"] {
        assert!(
            matches!(disable_item(env, id), Err(CatalogError::Unsafe(_))),
            "{id}"
        );
    }
    assert!(env.claude_dir.join("skills/real/SKILL.md").is_file());
    assert!(outside.is_file());
    assert!(!env.store_dir().join("skills/alias").exists());
}

#[test]
fn a_persons_own_unit_can_be_switched_but_never_deleted() {
    let w = tree_world();
    let env = &w.env;
    put(env, "skills/mine/SKILL.md", "# mine");
    disable_item(env, "skill:mine").unwrap();
    let entry = find(&tree_entries(env).unwrap(), "skill:mine").clone();
    assert_eq!(
        (
            entry.state,
            entry.owner,
            entry.can_uninstall,
            entry.can_toggle
        ),
        (EntryState::Disabled, Owner::You, false, true)
    );
    assert!(matches!(
        uninstall_item(env, "skill:mine"),
        Err(CatalogError::Personal(_))
    ));
    assert!(env.store_dir().join("skills/mine/SKILL.md").is_file());
    enable_item(env, "skill:mine").unwrap();
    assert_eq!(state_of(env, "skill:mine"), EntryState::Personal);
    assert!(matches!(
        uninstall_item(env, "skill:mine"),
        Err(CatalogError::Personal(_))
    ));
    assert!(env.claude_dir.join("skills/mine/SKILL.md").is_file());
    // Not in the registry and nowhere on disk is just unknown.
    assert!(matches!(
        uninstall_item(env, "skill:ghost"),
        Err(CatalogError::UnknownItem(_))
    ));
}

#[test]
fn enable_will_not_overwrite_what_now_sits_at_the_destination() {
    let w = tree_world();
    put(&w.env, "agents/mine.md", "# first");
    disable_item(&w.env, "agent:mine").unwrap();
    put(&w.env, "agents/mine.md", "# second");
    assert!(matches!(
        enable_item(&w.env, "agent:mine"),
        Err(CatalogError::Conflict(_))
    ));
    assert_eq!(
        fs::read_to_string(w.env.claude_dir.join("agents/mine.md")).unwrap(),
        "# second"
    );
    assert!(w.env.store_dir().join("agents/mine.md").is_file());
    assert!(disabled_json(&w.env)["disabled"]["agent:mine"].is_object());
}

#[test]
fn an_item_the_engine_shipped_uninstalls_without_a_lock_entry() {
    let w = tree_world();
    let env = &w.env;
    put(env, "agents/lead.md", "# lead");
    put(env, "agents/mine.md", "# mine");
    assert!(read_lock(env).items.is_empty());
    uninstall_item(env, "agent:lead").unwrap();
    assert!(!env.claude_dir.join("agents/lead.md").exists());
    assert!(env.claude_dir.join("agents/mine.md").is_file());
    assert_eq!(
        disabled_json(env)["removed"],
        serde_json::json!(["agent:lead"])
    );
    // Nothing of it is left, so there is nothing more to remove.
    assert!(matches!(
        uninstall_item(env, "agent:lead"),
        Err(CatalogError::UnknownItem(_))
    ));
}

#[test]
fn uninstalling_through_the_lock_records_removed() {
    let w = tree_world();
    install_item(&w.env, "rule:style").unwrap();
    uninstall_item(&w.env, "rule:style").unwrap();
    assert_eq!(
        disabled_json(&w.env)["removed"],
        serde_json::json!(["rule:style"])
    );
}

#[test]
fn install_clears_removed() {
    let w = tree_world();
    let env = &w.env;
    put(env, "agents/lead.md", "# lead");
    uninstall_item(env, "agent:lead").unwrap();
    put(env, "rules/style.md", "# style");
    uninstall_item(env, "rule:style").unwrap();
    assert_eq!(
        disabled_json(env)["removed"],
        serde_json::json!(["agent:lead", "rule:style"])
    );
    install_item(env, "agent:lead").unwrap();
    assert_eq!(
        disabled_json(env)["removed"],
        serde_json::json!(["rule:style"])
    );
    assert!(env.claude_dir.join("agents/lead.md").is_file());
}

#[test]
fn installing_replaces_what_the_store_held() {
    let w = tree_world();
    let env = &w.env;
    install_item(env, "skill:funnel").unwrap();
    disable_item(env, "skill:funnel").unwrap();
    install_item(env, "skill:funnel").unwrap();
    assert!(env.claude_dir.join("skills/funnel/SKILL.md").is_file());
    assert!(!env.store_dir().join("skills/funnel").exists());
    assert_eq!(disabled_json(env)["disabled"], serde_json::json!({}));
    assert_eq!(state_of(env, "skill:funnel"), EntryState::Installed);
}

#[test]
fn uninstalling_a_disabled_item_deletes_the_held_copy() {
    let w = tree_world();
    let env = &w.env;
    // With a lock entry.
    install_item(env, "skill:funnel").unwrap();
    disable_item(env, "skill:funnel").unwrap();
    uninstall_item(env, "skill:funnel").unwrap();
    assert!(!env.store_dir().join("skills/funnel").exists());
    let json = disabled_json(env);
    assert_eq!(json["disabled"], serde_json::json!({}));
    assert_eq!(json["removed"], serde_json::json!(["skill:funnel"]));
    assert_eq!(state_of(env, "skill:funnel"), EntryState::Available);
    // Without one: the engine shipped it.
    put(env, "agents/lead.md", "# lead");
    disable_item(env, "agent:lead").unwrap();
    uninstall_item(env, "agent:lead").unwrap();
    assert!(!env.store_dir().join("agents/lead.md").exists());
    assert_eq!(
        disabled_json(env)["removed"],
        serde_json::json!(["agent:lead", "skill:funnel"])
    );
}

#[test]
fn an_item_another_installed_item_needs_is_still_in_use() {
    let w = standard();
    install_item(&w.env, "pack:sales").unwrap();
    // `skill:funnel` is in the lock, so this is the existing check; the disk scan changes nothing.
    assert!(matches!(
        uninstall_item(&w.env, "skill:funnel"),
        Err(CatalogError::InUse(_, _))
    ));
    assert!(w.env.claude_dir.join("skills/funnel/SKILL.md").is_file());
}

#[test]
fn traversal_ids_are_refused_by_disable_and_enable() {
    let w = tree_world();
    let outside = w.env.claude_dir.parent().unwrap().join("outside.md");
    fs::write(&outside, "x").unwrap();
    for bad in [
        "skill:../outside",
        "agent:../../outside",
        "../agent:x",
        "skill:a/b",
        "agent:",
        "",
        "Skill:Funnel",
    ] {
        assert!(disable_item(&w.env, bad).is_err(), "{bad:?}");
        assert!(enable_item(&w.env, bad).is_err(), "{bad:?}");
    }
    assert!(outside.is_file());
    assert!(!w.env.store_dir().exists() || rel_files(&w.env.store_dir()).is_empty());
}

#[test]
fn a_tampered_record_cannot_steer_where_enable_writes() {
    let w = tree_world();
    let env = &w.env;
    put(env, "skills/ok/SKILL.md", "# ok");
    disable_item(env, "skill:ok").unwrap();
    // The record lies about the path; the id alone decides.
    let mut json = disabled_json(env);
    json["disabled"]["skill:ok"]["files"] = serde_json::json!(["../../evil"]);
    fs::write(env.disabled_path(), json.to_string()).unwrap();
    enable_item(env, "skill:ok").unwrap();
    assert!(env.claude_dir.join("skills/ok/SKILL.md").is_file());
    assert!(!env.claude_dir.parent().unwrap().join("evil").exists());
}

#[test]
fn the_entry_types_serialize_for_the_webview() {
    let w = tree_world();
    let tree = tree_entries(&w.env).unwrap();
    let json = serde_json::to_value(find(&tree, "hook:guard")).unwrap();
    assert_eq!(json["state"], "available");
    assert_eq!(json["owner"], "kun");
    assert_eq!(json["dir"], "hooks");
    assert!(json.get("title_ar").is_none());
    assert_eq!(json["can_install"], true);
}
