// node --test src/components/shell/nav.test.ts (Node strips the types; no runner dependency).
import assert from "node:assert/strict"
import { test } from "node:test"

import { APP_STAGES, isApp } from "../apps/registry.ts"
import { backHref, buildTree, dirOfItem, splitPath, stageCounts, trailOf } from "./nav.ts"

const q = (s: string) => new URLSearchParams(s)
const trail = (path: string, search = "") => trailOf(path, q(search))

test("splitPath drops the slash and the locale", () => {
  assert.deepEqual(splitPath("/ar/setup/"), { lang: "ar", rest: "/setup" })
  assert.deepEqual(splitPath("/en"), { lang: "en", rest: "" })
})

test("sessions, and places outside the tree", () => {
  assert.deepEqual(trail("/en/sessions/"), { parent: "sessions", child: null, leaf: null })
  assert.deepEqual(trail("/en/"), { parent: null, child: null, leaf: null })
  assert.deepEqual(trail("/en/nowhere/"), { parent: null, child: null, leaf: null })
})

test("setup: stage and step ride on the query", () => {
  assert.deepEqual(trail("/en/setup/"), { parent: "setup", child: null, leaf: null })
  assert.deepEqual(trail("/en/setup/", "stage=repos"), { parent: "setup", child: "repos", leaf: null })
  assert.deepEqual(trail("/ar/setup/", "stage=repos&step=all"), { parent: "setup", child: "repos", leaf: "all" })
  assert.equal(trail("/en/setup/", "step=all").leaf, null)
})

test("library: only the five folders count, and an item needs its folder", () => {
  assert.deepEqual(trail("/en/library/"), { parent: "library", child: null, leaf: null })
  assert.deepEqual(trail("/en/library/", "dir=skills"), { parent: "library", child: "skills", leaf: null })
  assert.deepEqual(trail("/en/library/", "dir=skills&item=skill:funnel"), { parent: "library", child: "skills", leaf: "skill:funnel" })
  assert.deepEqual(trail("/en/library/", "dir=bogus&item=skill:funnel"), { parent: "library", child: null, leaf: null })
})

test("plugins: the item rides on the query and is the child", () => {
  assert.deepEqual(trail("/en/plugins/"), { parent: "plugins", child: null, leaf: null })
  assert.deepEqual(trail("/ar/plugins/", "item=blender"), { parent: "plugins", child: "blender", leaf: null })
})

test("apps: the item is a readiness step id", () => {
  assert.deepEqual(trail("/en/apps/"), { parent: "apps", child: null, leaf: null })
  assert.deepEqual(trail("/ar/apps/", "item=claude.cli"), { parent: "apps", child: "claude.cli", leaf: null })
})

test("settings: the view and the three routes are its children", () => {
  assert.deepEqual(trail("/en/settings/"), { parent: "settings", child: null, leaf: null })
  assert.equal(trail("/en/settings/", "view=general").child, "general")
  assert.equal(trail("/en/settings/", "view=other").child, null)
  for (const r of ["config", "health", "engine"]) assert.deepEqual(trail(`/en/${r}/`), { parent: "settings", child: r, leaf: null })
})

test("stage counts skip `skip` steps and count pass, fixed, updated as done", () => {
  const c = stageCounts([
    { stage: "a", status: "pass" },
    { stage: "a", status: "fixed" },
    { stage: "a", status: "updated" },
    { stage: "a", status: "fail" },
    { stage: "a", status: "skip" },
    { stage: "b", status: "skip" },
  ])
  assert.deepEqual(c, { a: { done: 3, total: 4 } })
})

test("tree: gating, and setup's children without the app stages", () => {
  const stages = ["identity", "foundation", "agents", "repos", "tools"].map((id) => ({ id, title: null }))
  const input = { isAr: false, inApp: false, engine: false, stages, steps: [] }
  const ids = (n: ReturnType<typeof buildTree>, p: string) => n.find((x) => x.id === p)?.children?.map((c) => c.id)
  assert.deepEqual(ids(buildTree(input), "settings"), ["general"])
  assert.deepEqual(ids(buildTree({ ...input, inApp: true }), "settings"), ["general", "config", "health"])
  assert.deepEqual(ids(buildTree({ ...input, inApp: true, engine: true }), "settings"), ["general", "config", "health", "engine"])
  assert.deepEqual(ids(buildTree(input), "setup"), ["identity", "repos"])
  assert.deepEqual(ids(buildTree(input), "library"), ["skills", "agents", "rules", "hooks", "departments"])
})

test("tree: the six parents in the order a person works", () => {
  const input = { isAr: false, inApp: false, engine: false, stages: [], steps: [] }
  const tree = buildTree(input)
  assert.deepEqual(tree.map((n) => n.id), ["sessions", "plugins", "apps", "library", "setup", "settings"])
  const kids = (p: string) => tree.find((n) => n.id === p)?.children ?? []
  assert.ok(kids("plugins").some((k) => k.id === "blender" && k.href("en") === "/en/plugins/?item=blender"))
  assert.ok(kids("apps").some((k) => k.id === "claude.cli" && k.href("en") === "/en/apps/?item=claude.cli"))
  assert.ok(!kids("apps").some((k) => k.id === "claude.launcher"))
})

test("app stages are Apps', every app step sits in one, and the hidden checks are not apps", () => {
  assert.deepEqual([...APP_STAGES], ["foundation", "agents", "tools"])
  assert.equal(isApp({ id: "tool.blender", stage: "tools" }), true)
  assert.equal(isApp({ id: "claude.launcher", stage: "agents" }), false)
  assert.equal(isApp({ id: "gh.login", stage: "identity" }), false)
})

test("an item id names its folder", () => {
  assert.equal(dirOfItem("pack:sales"), "departments")
  assert.equal(dirOfItem("hook:x"), "hooks")
  assert.equal(dirOfItem("nope:x"), null)
})

test("back goes one level up the breadcrumb, and from a parent to the chat", () => {
  const back = (path: string, search = "") => backHref(path, q(search))
  assert.equal(back("/en/setup/", "stage=core&step=git"), "/en/setup/?stage=core")
  assert.equal(back("/en/setup/", "stage=core"), "/en/setup/")
  assert.equal(back("/en/setup/"), "/en/")
  assert.equal(back("/ar/library/", "dir=skills&item=skill:funnel"), "/ar/library/?dir=skills")
  assert.equal(back("/ar/library/", "dir=skills"), "/ar/library/")
  assert.equal(back("/ar/plugins/", "item=blender"), "/ar/plugins/")
  assert.equal(back("/ar/plugins/"), "/ar/")
  assert.equal(back("/en/config/"), "/en/settings/")
  assert.equal(back("/en/sessions/"), "/en/")
})
