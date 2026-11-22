// node --test src/components/chat/intake.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import type { Question } from "../pack/types.ts"
import {
  answeredSummary,
  briefMessage,
  briefTitle,
  canStartFromText,
  dismissOpenIntake,
  foldBrief,
  intakeMessage,
  intakeOutcome,
  introLine,
  settleIntake,
  userMessage,
} from "./intake-pure.ts"

const q = (slot: string, opts: [string, string, string][], over: Partial<Question> = {}): Question => ({
  slot,
  kind: "choice",
  q: { en: slot, ar: slot },
  options: opts.map(([value, en, ar]) => ({ value, label: { en, ar } })),
  default: null,
  current: null,
  ...over,
})
const three = [
  q("audience", [["owners", "School owners", "ملاك المدارس"], ["teachers", "Teachers", "المعلمون"]], { default: "owners" }),
  q("market", [["sa", "Saudi Arabia", "السعودية"], ["sd", "Sudan", "السودان"]], { current: "sd" }),
  q("cut", [["film", "2-minute film, Facebook 9:16", "فيلم دقيقتان، فيسبوك"]]),
]
const open = () => intakeMessage({ n: 1, dept: "video", id: "video.film.plan", set: { new: "1" }, questions: three, intro: null }, false)

test("the intro line follows the app language and falls back when the formula has none", () => {
  const w = { en: "An intro film.", ar: "فيلم تعريفي." }
  assert.equal(introLine(w, false), "An intro film.")
  assert.equal(introLine(w, true), "فيلم تعريفي.")
  assert.equal(introLine({ en: "Only English", ar: "" }, true), "Only English")
  assert.match(introLine(null, false), /quick questions/)
  assert.match(introLine({ en: " ", ar: "" }, true), /أسئلة/)
})

test("the intake message is local, open, and keeps every key of the set", () => {
  const m = open()
  assert.equal(m.role, "assistant")
  assert.equal(m.kind, "intake")
  assert.equal(m.intake?.status, "open")
  assert.deepEqual(m.intake?.set, { new: "1" })
  assert.equal(userMessage("New film").kind, undefined)
})

test("the answered summary reads like Claude Code, in either language", () => {
  assert.equal(answeredSummary(three, { audience: "teachers", market: "sa", cut: "film" }, false), "Audience: Teachers · Market: Saudi Arabia · Cut: 2-minute film, Facebook 9:16")
  assert.equal(answeredSummary(three, { audience: "teachers", market: "sa", cut: "film" }, true), "الجمهور: المعلمون · السوق: السعودية · Cut: فيلم دقيقتان، فيسبوك")
  // Nothing answered: the current or default value shows; a typed value shows as typed; a slot with no value is left out.
  assert.equal(answeredSummary(three, {}, false), "Audience: School owners · Market: Sudan")
  assert.equal(answeredSummary(three, { cut: "a typed cut" }, false), "Audience: School owners · Market: Sudan · Cut: a typed cut")
})

test("the outcome line: the summary, the defaults note, or skipped", () => {
  const m = open()
  assert.equal(intakeOutcome({ ...m.intake!, status: "answered", answers: { market: "sa" } }, false), "Audience: School owners · Market: Saudi Arabia")
  assert.equal(intakeOutcome({ ...m.intake!, status: "answered", questions: [q("x", [])] }, false), "Defaults kept")
  assert.match(intakeOutcome({ ...m.intake!, status: "skipped" }, false), /Skipped/)
  assert.match(intakeOutcome({ ...m.intake!, status: "skipped" }, true), /تُجوهلت/)
})

test("typing during an open intake dismisses it and keeps the defaults", () => {
  const list = [userMessage("New film"), open()]
  const next = dismissOpenIntake(list)
  assert.equal(next[1]!.intake?.status, "skipped")
  assert.equal(list[1]!.intake?.status, "open")
  // Nothing open: the same list comes back.
  assert.equal(dismissOpenIntake(next), next)
})

test("settling keeps the answers and only touches the open intake of that token", () => {
  const list = [userMessage("New film"), open()]
  const done = settleIntake(list, 1, { market: "sa" })
  assert.equal(done[1]!.intake?.status, "answered")
  assert.deepEqual(done[1]!.intake?.answers, { market: "sa" })
  assert.equal(settleIntake(list, 2, {})[1]!.intake?.status, "open")
})

test("Enter on typed text opens an intake only on a fresh chat, never mid-run or mid-intake", () => {
  assert.equal(canStartFromText([], "plan an intro film for the school"), true)
  assert.equal(canStartFromText([], "   "), false)
  assert.equal(canStartFromText([], "/video something"), false)
  assert.equal(canStartFromText([userMessage("New film"), open()], "more please"), false)
  const settled = settleIntake([userMessage("New film"), open()], 1, {})
  assert.equal(canStartFromText([...settled, briefMessage({ id: "a", dept: "video", title: "t", prompt: "p", route: null, record: "none", model: null, effort: null, label: "x" })], "another film"), true)
  const ran = [{ ...userMessage("hi") }, { role: "assistant" as const, text: "hello", tools: [], live: null }]
  assert.equal(canStartFromText(ran, "plan an intro film"), false)
})

test("the brief title names the formula and the model; the prompt folds to two lines", () => {
  assert.equal(briefTitle("Intro film", "opus", false), "Brief · Intro film · Opus 5.5")
  assert.equal(briefTitle("فيلم تعريفي", "sonnet", true), "الموجز · فيلم تعريفي · Sonnet 5.5")
  assert.equal(briefTitle("Intro film", null, false), "Brief · Intro film · Claude")
  assert.deepEqual(foldBrief("one\ntwo\nthree"), { head: "one\ntwo", hasMore: true })
  assert.deepEqual(foldBrief("one\ntwo"), { head: "one\ntwo", hasMore: false })
  assert.deepEqual(foldBrief("only"), { head: "only", hasMore: false })
})
