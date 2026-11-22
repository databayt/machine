// node --test src/components/chat/questions.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import type { Question } from "../pack/types.ts"
import { answered, answersToSet, initState, optionLabel, reduce, rowCount, shownValue, slotTitle, tabDelta } from "./questions-pure.ts"

const words = (s: string) => ({ en: s, ar: s })
const q = (slot: string, values: string[], over: Partial<Question> = {}): Question => ({
  slot,
  kind: "choice",
  q: words(slot),
  options: values.map((v) => ({ value: v, label: { en: `${v} en`, ar: `${v} ar` } })),
  default: null,
  current: null,
  ...over,
})

const three = [q("market", ["sd", "sa", "ae"], { default: "sd", current: "sd" }), q("audience", ["parents", "teachers"]), q("length", ["30", "60"])]

test("the cursor starts on the current value", () => {
  assert.equal(initState([q("m", ["a", "b", "c"], { current: "c" })]).cursor, 2)
  assert.equal(initState([q("m", ["a", "b"], { default: "b" })]).cursor, 1)
  assert.equal(initState([q("m", ["a", "b"])]).cursor, 0)
})

test("arrows move through the options and the type row, and wrap", () => {
  let s = initState([q("m", ["a", "b"])])
  assert.equal(rowCount(s.questions[0]!), 3)
  s = reduce(s, { type: "move", delta: 1 })
  s = reduce(s, { type: "move", delta: 1 })
  assert.equal(s.cursor, 2)
  s = reduce(s, { type: "move", delta: 1 })
  assert.equal(s.cursor, 0)
  s = reduce(s, { type: "move", delta: -1 })
  assert.equal(s.cursor, 2)
})

test("Enter picks and advances; the last question lands on Submit", () => {
  let s = initState(three)
  s = reduce(s, { type: "move", delta: 1 })
  s = reduce(s, { type: "enter" })
  assert.deepEqual(s.answers, { market: "sa" })
  assert.equal(s.tab, 1)
  s = reduce(s, { type: "digit", n: 2 })
  assert.equal(s.tab, 2)
  s = reduce(s, { type: "digit", n: 1 })
  assert.equal(s.tab, 3)
  assert.equal(s.done, null)
  s = reduce(s, { type: "enter" })
  assert.equal(s.done, "submit")
  assert.deepEqual(answersToSet(s), { market: "sa", audience: "teachers", length: "30" })
})

test("a single question submits as soon as it is answered", () => {
  const s = reduce(initState([q("m", ["a", "b"])]), { type: "digit", n: 2 })
  assert.equal(s.done, "submit")
  assert.deepEqual(answersToSet(s), { m: "b" })
})

test("a number key past the rows does nothing", () => {
  const s = initState([q("m", ["a"])])
  assert.equal(reduce(s, { type: "digit", n: 9 }), s)
})

test("Type something takes free text; an empty text goes back", () => {
  let s = initState([q("m", ["a", "b"]), q("n", ["x"])])
  s = reduce(s, { type: "digit", n: 3 })
  assert.equal(s.typing, true)
  s = reduce(s, { type: "type", text: "Libya" })
  s = reduce(s, { type: "enter" })
  assert.deepEqual(s.answers, { m: "Libya" })
  assert.equal(s.tab, 1)
  s = reduce(s, { type: "digit", n: 2 })
  assert.equal(s.typing, true)
  assert.equal(reduce(s, { type: "enter" }).typing, false)
})

test("Esc leaves typing first, then closes with the defaults", () => {
  let s = initState(three)
  s = reduce(s, { type: "row", row: 3 })
  assert.equal(s.typing, true)
  s = reduce(s, { type: "esc" })
  assert.equal(s.typing, false)
  assert.equal(s.done, null)
  s = reduce(s, { type: "esc" })
  assert.equal(s.done, "close")
  assert.deepEqual(answersToSet({ ...s, answers: { market: "sa" } }), {})
})

test("Tab and goto move between tabs, wrap, and keep each tab's pick", () => {
  let s = initState(three)
  s = reduce(s, { type: "tab", delta: -1 })
  assert.equal(s.tab, 3)
  s = reduce(s, { type: "tab", delta: 1 })
  assert.equal(s.tab, 0)
  s = reduce(s, { type: "digit", n: 3 })
  s = reduce(s, { type: "goto", tab: 0 })
  assert.equal(s.cursor, 2)
  assert.equal(answered(s, 0), true)
  assert.equal(answered(s, 1), false)
  assert.equal(shownValue(s.questions[0]!, s.answers), "ae")
})

test("on the Submit tab only Enter and Esc act", () => {
  let s = reduce(initState(three), { type: "goto", tab: 3 })
  assert.equal(reduce(s, { type: "move", delta: 1 }), s)
  assert.equal(reduce(s, { type: "digit", n: 1 }), s)
  s = reduce(s, { type: "enter" })
  assert.equal(s.done, "submit")
})

test("the arrows mirror in Arabic", () => {
  assert.equal(tabDelta("ArrowRight", false), 1)
  assert.equal(tabDelta("ArrowRight", true), -1)
  assert.equal(tabDelta("ArrowLeft", true), 1)
  assert.equal(tabDelta("x", true), 0)
})

test("titles and labels follow the language", () => {
  assert.equal(slotTitle("market", false), "Market")
  assert.equal(slotTitle("market", true), "السوق")
  assert.equal(slotTitle("target_audience", false), "Target audience")
  assert.equal(optionLabel({ value: "v", label: { en: "E", ar: "A" } }, true), "A")
  assert.equal(optionLabel({ value: "v", label: { en: "", ar: "" } }, false), "v")
})
