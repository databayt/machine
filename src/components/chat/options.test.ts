// node --test src/components/chat/options.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import type { Board, BoardOption, QuickAction } from "../pack/types.ts"
import { MATCH_MIN, homeOptions, iconKey, optionsFromBoard, optionsFromQuick, usableMatch } from "./options.ts"

const opt = (over: Partial<BoardOption>): BoardOption => ({
  role: "start",
  id: "video.film.plan",
  label_en: "New film",
  label_ar: "فيلم جديد",
  icon: "Film",
  route: "plan",
  record: "issue",
  score: 2,
  why: [],
  set: {},
  ...over,
})
const board = (options: BoardOption[]): Board => ({ department: "video", repo: "hogwarts", options, error: null })
const quick = (over: Partial<QuickAction>): QuickAction => ({
  id: null,
  label_ar: "عرض",
  label_en: "Proposal",
  prompt: "Draft a proposal",
  description_en: "",
  description_ar: "",
  repos: [],
  route: null,
  ...over,
})

test("board options keep the script's order, at most three, ids once", () => {
  const b = board([
    opt({ id: "a.one", label_en: "One" }),
    opt({ id: "a.one", label_en: "Again" }),
    opt({ id: "a.two", label_en: "Two" }),
    opt({ id: "a.three", label_en: "Three" }),
    opt({ id: "a.four", label_en: "Four" }),
  ])
  assert.deepEqual(optionsFromBoard(b, false).map((o) => o.label), ["One", "Two", "Three"])
})

test("a board option carries its formula, slots, icon and route; a tap never refines", () => {
  const [o] = optionsFromBoard(board([opt({ id: "video.film.continue", set: { project: "p-v001" }, icon: "Play", route: "do" })]), false)
  assert.equal(o!.formula, "video.film.continue")
  assert.equal(o!.logId, "video.film.continue")
  assert.deepEqual(o!.set, { project: "p-v001" })
  assert.equal(o!.icon, "Play")
  assert.equal(o!.route, "do")
  assert.equal(o!.refine, false)
  assert.equal(o!.prompt, null)
})

test("labels follow the language and fall back to the other one, then the id", () => {
  assert.equal(optionsFromBoard(board([opt({})]), true)[0]!.label, "فيلم جديد")
  assert.equal(optionsFromBoard(board([opt({ label_ar: "" })]), true)[0]!.label, "New film")
  assert.equal(optionsFromBoard(board([opt({ label_ar: "", label_en: "" })]), true)[0]!.label, "video.film.plan")
  assert.equal(optionsFromBoard(board([opt({})]), true)[0]!.labelEn, "New film")
})

test("an empty or missing board falls back to today's quick actions", () => {
  const q = [quick({ label_en: "A", prompt: "a" }), quick({ label_en: "B", prompt: "b", repos: ["hogwarts"] })]
  for (const b of [null, board([])]) {
    const out = homeOptions(b, q, "", false)
    assert.deepEqual(out.map((o) => o.label), ["A"], "no repo: the any-repo actions")
    assert.equal(out[0]!.formula, null)
    assert.equal(out[0]!.refine, true, "a quick action still goes through refine")
    assert.equal(out[0]!.prompt, "a")
  }
  assert.deepEqual(homeOptions(board([]), q, "hogwarts", false).map((o) => o.label), ["B"], "the repo's own actions win")
  assert.deepEqual(homeOptions(null, q, "mkan", false).map((o) => o.label), ["A"], "no action for the repo: any-repo ones")
  assert.equal(homeOptions(board([opt({})]), q, "", false)[0]!.formula, "video.film.plan", "a non-empty board wins")
})

test("quick actions are capped at three and carry their id and route for the log", () => {
  const q = ["a", "b", "c", "d"].map((x) => quick({ label_en: x, prompt: x, id: x === "a" ? "video.reel.footage" : null, route: x === "a" ? "do" : null }))
  const out = optionsFromQuick(q, "", false)
  assert.equal(out.length, 3)
  assert.equal(out[0]!.logId, "video.reel.footage")
  assert.equal(out[0]!.route, "do")
  assert.equal(out[1]!.logId, null)
  assert.equal(new Set(out.map((o) => o.key)).size, 3)
})

test("icon names are matched without their case, suffix or dashes", () => {
  assert.equal(iconKey("Play"), "play")
  assert.equal(iconKey("PlayIcon"), "play")
  assert.equal(iconKey("scroll-text"), "scrolltext")
  assert.equal(iconKey("ScrollText"), "scrolltext")
  assert.equal(iconKey(null), "")
})

test("typed text is filled by the station only with an id and enough confidence", () => {
  assert.equal(MATCH_MIN, 0.6)
  assert.equal(usableMatch({ id: "video.film.plan", confidence: 0.92 }), true)
  assert.equal(usableMatch({ id: "video.film.plan", confidence: 0.6 }), true)
  assert.equal(usableMatch({ id: "video.film.plan", confidence: 0.59 }), false)
  assert.equal(usableMatch({ id: null, confidence: 0.99 }), false)
  assert.equal(usableMatch({ id: "x.y", confidence: Number.NaN }), false)
  assert.equal(usableMatch(null), false)
})
