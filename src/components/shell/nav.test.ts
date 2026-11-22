// node --test src/components/shell/nav.test.ts (Node strips the types; no runner dependency).
import assert from "node:assert/strict"
import { test } from "node:test"

import { backHref, buildTree, dirOfItem, splitPath, stageCounts, trailOf } from "./nav.ts"

const q = (s: string) => new URLSearchParams(s)
const trail = (path: string, search = "") => trailOf(path, q(search))

test("splitPath drops the slash and the locale", () => {
  assert.deepEqual(splitPath("/ar/install/"), { lang: "ar", rest: "/install" })
  assert.deepEqual(splitPath("/en"), { lang: "en", rest: "" })
})

test("sessions, and places outside the tree", () => {
  assert.deepEqual(trail("/en/sessions/"), { parent: "sessions", child: null, leaf: null })
  assert.deepEqual(trail("/en/"), { parent: null, child: null, leaf: null })
  assert.deepEqual(trail("/en/nowhere/"), { parent: null, child: null, leaf: null })
})

test("install: stage and step ride on the query", () => {
  assert.deepEqual(trail("/en/install/"), { parent: "install", child: null, leaf: null })
  assert.deepEqual(trail("/en/install/", "stage=repos"), { parent: "install", child: "repos", leaf: null })
  assert.deepEqual(trail("/ar/install/", "stage=repos&step=all"), { parent: "install", child: "repos", leaf: "all" })
  assert.equal(trail("/en/install/", "step=all").leaf, null)
})

test(".claude: only the six folders count, and an item needs its folder", () => {
  assert.deepEqual(trail("/en/catalog/"), { parent: "claude", child: null, leaf: null })
  assert.deepEqual(trail("/en/catalog/", "dir=skills"), { parent: "claude", child: "skills", leaf: null })
  assert.deepEqual(trail("/en/catalog/", "dir=skills&item=skill:funnel"), { parent: "claude", child: "skills", leaf: "skill:funnel" })
  assert.deepEqual(trail("/en/catalog/", "dir=bogus&item=skill:funnel"), { parent: "claude", child: null, leaf: null })
})

test("connectors: the item rides on the query and is the child", () => {
  assert.deepEqual(trail("/en/connectors/"), { parent: "connectors", child: null, leaf: null })
  assert.deepEqual(trail("/ar/connectors/", "item=blender"), { parent: "connectors", child: "blender", leaf: null })
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

test("tree: gating and install's dynamic children", () => {
  const input = { isAr: false, inApp: false, engine: false, stages: [{ id: "repos", title: null }], steps: [] }
  const ids = (n: ReturnType<typeof buildTree>, p: string) => n.find((x) => x.id === p)?.children?.map((c) => c.id)
  assert.deepEqual(ids(buildTree(input), "settings"), ["general"])
  assert.deepEqual(ids(buildTree({ ...input, inApp: true }), "settings"), ["general", "config", "health"])
  assert.deepEqual(ids(buildTree({ ...input, inApp: true, engine: true }), "settings"), ["general", "config", "health", "engine"])
  assert.deepEqual(ids(buildTree(input), "install"), ["repos"])
  assert.deepEqual(ids(buildTree(input), "claude"), ["skills", "agents", "rules", "hooks", "departments", "mcp"])
})

test("tree: connectors sit between .claude and settings", () => {
  const input = { isAr: false, inApp: false, engine: false, stages: [], steps: [] }
  const ids = buildTree(input).map((n) => n.id)
  assert.equal(ids.indexOf("connectors"), ids.indexOf("claude") + 1)
  assert.equal(ids.indexOf("settings"), ids.indexOf("connectors") + 1)
  const kids = buildTree(input).find((n) => n.id === "connectors")?.children ?? []
  assert.ok(kids.some((k) => k.id === "blender" && k.href("en") === "/en/connectors/?item=blender"))
})

test("an item id names its folder", () => {
  assert.equal(dirOfItem("pack:sales"), "departments")
  assert.equal(dirOfItem("hook:x"), "hooks")
  assert.equal(dirOfItem("nope:x"), null)
})

test("back goes one level up the breadcrumb, and from a parent to the chat", () => {
  const back = (path: string, search = "") => backHref(path, q(search))
  assert.equal(back("/en/install/", "stage=core&step=git"), "/en/install/?stage=core")
  assert.equal(back("/en/install/", "stage=core"), "/en/install/")
  assert.equal(back("/en/install/"), "/en/")
  assert.equal(back("/ar/catalog/", "dir=skills&item=skill:funnel"), "/ar/catalog/?dir=skills")
  assert.equal(back("/ar/catalog/", "dir=skills"), "/ar/catalog/")
  assert.equal(back("/ar/connectors/", "item=blender"), "/ar/connectors/")
  assert.equal(back("/ar/connectors/"), "/ar/")
  assert.equal(back("/en/config/"), "/en/settings/")
  assert.equal(back("/en/sessions/"), "/en/")
})
