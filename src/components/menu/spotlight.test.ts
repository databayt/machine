// node --test src/components/menu/spotlight.test.ts (Node strips the types; no runner dependency).
import assert from "node:assert/strict"
import { test } from "node:test"

import type { Task } from "../tasks/types.ts"
import { MAX_SPOTLIGHT, rankSpotlight, type SpotlightInput } from "./spotlight.ts"

const task = (n: number, over: Partial<Task> = {}): Task => ({
  id: `databayt/kun#${n}`,
  repo: "kun",
  number: n,
  title: `Task ${n}`,
  url: "",
  department: "sales",
  status: "todo",
  holder: null,
  priority: null,
  sprint: "Sprint 14",
  order: n,
  auto: false,
  kun: { repos: [], items: [], after: [], driver: null },
  afterOpen: [],
  body: "",
  updatedAt: "",
  ...over,
})

const view = (tasks: Task[], readiness: Record<string, { state: "ready" | "missing" | "blocked"; missing: string[] }> = {}) =>
  ({
    sprint: { title: "Sprint 14", goal: "Close three pilots", endsAt: null },
    mine: [],
    board: tasks,
    inProgress: 0,
    wipLimit: 2,
    readiness,
  }) as SpotlightInput["view"]

const base = (over: Partial<SpotlightInput>): SpotlightInput => ({
  lang: "en",
  view: null,
  tasks: [],
  login: "me",
  unread: 0,
  health: null,
  ...over,
})

test("not connected: the banner still says so", () => {
  assert.deepEqual(rankSpotlight(base({})).map((i) => i.kind), ["connect"])
})

test("nothing to say: connected, no tasks, no lanes, nothing to know", () => {
  const view = { sprint: null, mine: [], board: [], inProgress: 0, wipLimit: 1, readiness: {} }
  assert.deepEqual(rankSpotlight(base({ view: view as unknown as SpotlightInput["view"] })), [])
})

test("to-do first: held, then ready by priority, then clone; to-know after", () => {
  const held = task(1, { holder: "me" })
  const p1 = task(2, { priority: 1 })
  const p0 = task(3, { priority: 0 })
  const far = task(4)
  const v = view([held, p1, p0, far], {
    [held.id]: { state: "ready", missing: [] },
    [p1.id]: { state: "ready", missing: [] },
    [p0.id]: { state: "ready", missing: [] },
    [far.id]: { state: "missing", missing: ["repo:hogwarts"] },
  })
  const out = rankSpotlight(base({ view: v, tasks: [held, p1, p0, far], unread: 2 }))
  assert.equal(out.length, MAX_SPOTLIGHT)
  assert.deepEqual(
    out.map((i) => i.kind),
    ["continue", "start", "start", "sprint"],
  )
  assert.equal(out[1].taskId, p0.id)
  assert.equal(out[0].side, "do")
  assert.equal(out.at(-1)?.side, "know")
})

test("a missing repo becomes one Clone item pointing at Repos", () => {
  const a = task(5)
  const b = task(6)
  const v = view([a, b], {
    [a.id]: { state: "missing", missing: ["repo:hogwarts"] },
    [b.id]: { state: "missing", missing: ["repo:hogwarts"] },
  })
  const clone = rankSpotlight(base({ view: v, tasks: [a, b] })).filter((i) => i.kind === "clone")
  assert.equal(clone.length, 1)
  assert.equal(clone[0].mark, "hogwarts")
  assert.equal(clone[0].href, "/install/?stage=repos&step=all")
  assert.match(clone[0].template, /#5\b/)
})

test("tasks held by someone else, and blocked ones, are not offered", () => {
  const other = task(7, { holder: "ali" })
  const blocked = task(8, { afterOpen: ["databayt/kun#1"] })
  const v = view([other, blocked], { [blocked.id]: { state: "blocked", missing: ["after:databayt/kun#1"] } })
  assert.deepEqual(rankSpotlight(base({ view: v, tasks: [other, blocked] })).map((i) => i.kind), ["sprint"])
})

test("unread notes and unhealed health; healed or ok items stay out", () => {
  const health = {
    checkedAt: "",
    items: [
      { key: "disk", ok: true, detail: "fine", healed: false },
      { key: "kun", ok: false, detail: "Kun is 3 commits behind", healed: false },
      { key: "driver", ok: false, detail: "fixed", healed: true },
    ],
  }
  const out = rankSpotlight(base({ view: view([]), unread: 1, health }))
  assert.deepEqual(out.map((i) => i.kind), ["sprint", "notes", "health"])
  assert.equal(out[1].mark, "1 note")
  assert.equal(out[2].href, "/engine/")
  assert.equal(out[2].mark, "Kun is 3 commits behind")
})

test("titles cannot reopen the emphasis; Arabic templates; dedupe", () => {
  const t = task(9, { title: "Fix **bold** {mark}" })
  const v = view([t], { [t.id]: { state: "ready", missing: [] } })
  const [first] = rankSpotlight(base({ lang: "ar", view: v, tasks: [t, t] }))
  assert.equal(first.template.split("**").length, 3)
  assert.ok(!first.template.includes("Fix **"))
  assert.match(first.template, /^\*\*ابدأ/)
  assert.equal(rankSpotlight(base({ view: v, tasks: [t, t] })).filter((i) => i.kind === "start").length, 1)
})

test("never empty: no tasks and no live sprint → the department's lanes, then connect", () => {
  const items = rankSpotlight({
    lang: "en",
    view: null,
    tasks: [],
    login: "moed",
    unread: 0,
    health: null,
    lanes: [
      { label: "Quote a school", prompt: "quote" },
      { label: "Who to follow up", prompt: "follow" },
      { label: "Find leads", prompt: "find" },
    ],
    department: "Sales",
  })
  assert.deepEqual(
    items.map((i) => i.kind),
    ["lane", "lane", "connect"],
  )
  assert.equal(items[0].mark, "Quote a school")
  assert.equal(items[2].mark, "Sales")
  assert.equal(items[2].href, "/settings/")
})

test("every sentence fits the banner's two lines", () => {
  const long = "A very long task title that goes on and on about drafting the quarterly sprint report"
  const items = rankSpotlight({
    lang: "en",
    view: { sprint: { title: "Sprint 14", goal: long, endsAt: null }, mine: [], board: [], inProgress: 0, wipLimit: 1, readiness: {} } as unknown as SpotlightInput["view"],
    tasks: [task(9, { title: long, holder: "me" }), task(10, { title: long })],
    login: "me",
    unread: 3,
    health: null,
    lanes: [{ label: "Quote a school", prompt: "q", detail: long }],
  })
  assert.ok(items.length > 0)
  for (const i of items) {
    const sentence = i.template.replace(/\*\*/g, "").replace("{mark}", i.mark)
    assert.ok(sentence.length <= 80, `${sentence.length}: ${sentence}`)
  }
})
