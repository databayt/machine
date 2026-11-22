// node --test src/components/chat/wizard.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import type { AskQuestion } from "./ask-pure.ts"
import { answerOf, collect, direct, initState, isAnswered, onReview, reduce, tabCount, tabDelta, type WAction, type WState } from "./wizard-pure.ts"

const opt = (label: string) => ({ label, description: `${label} desc` })
const single: AskQuestion = { question: "Who is it for?", header: "Audience", multiSelect: false, options: [opt("Owners"), opt("Teachers"), opt("Parents")] }
const multi: AskQuestion = { question: "Which platforms?", header: "Platforms", multiSelect: true, options: [opt("Facebook"), opt("TikTok"), opt("YouTube")] }
const run = (s: WState, ...a: WAction[]) => a.reduce(reduce, s)

test("one single-select question submits as soon as an option is chosen", () => {
  const s = initState([single])
  assert.ok(direct(s.questions) && tabCount(s) === 1)
  const r = run(s, { type: "move", delta: 1 }, { type: "enter" })
  assert.equal(r.done, "submit")
  assert.deepEqual(collect(r), [{ header: "Audience", question: "Who is it for?", answer: "Teachers" }])
})

test("digits choose, and the rows after the options are Type something and Chat about this", () => {
  const s = initState([single])
  assert.equal(run(s, { type: "digit", n: 3 }).done, "submit")
  // 4 = Type something: opens the input; 5 = Chat about this: closes the wizard.
  const t = run(s, { type: "digit", n: 4 })
  assert.ok(t.typing && !t.done)
  assert.equal(run(s, { type: "digit", n: 5 }).done, "chat")
  assert.equal(run(s, { type: "digit", n: 6 }), s)
  assert.equal(run(s, { type: "move", delta: -1 }, { type: "move", delta: -1 }, { type: "enter" }).cursor, 3)
})

test("typing: the text is the answer; empty text closes the input", () => {
  const s = run(initState([single]), { type: "digit", n: 4 }, { type: "type", text: "Investors" })
  assert.equal(run(s, { type: "enter" }).done, "submit")
  assert.equal(collect(run(s, { type: "enter" }))[0]!.answer, "Investors")
  const empty = run(initState([single]), { type: "digit", n: 4 }, { type: "enter" })
  assert.ok(!empty.typing && !empty.done)
  // Typing outside the input does nothing; Esc closes the input first, then cancels.
  assert.equal(reduce(initState([single]), { type: "type", text: "x" }).typed[0], "")
  const open = run(initState([single]), { type: "digit", n: 4 })
  assert.ok(!reduce(open, { type: "esc" }).typing && !reduce(open, { type: "esc" }).done)
  assert.equal(run(open, { type: "esc" }, { type: "esc" }).done, "cancel")
})

test("several questions: pick, move on, the review comes last and Submit sends the answers", () => {
  let s = initState([single, multi])
  assert.equal(tabCount(s), 3)
  s = run(s, { type: "digit", n: 1 })
  assert.equal(s.tab, 1)
  assert.equal(s.done, null)
  s = run(s, { type: "space" }, { type: "move", delta: 2 }, { type: "space" })
  assert.equal(answerOf(s, 1), "Facebook, YouTube")
  s = run(s, { type: "enter" })
  assert.ok(onReview(s))
  assert.equal(s.cursor, 0)
  const done = run(s, { type: "enter" })
  assert.equal(done.done, "submit")
  assert.deepEqual(collect(done).map((a) => a.answer), ["Owners", "Facebook, YouTube"])
})

test("multi-select: Space toggles, Enter on an option goes on, digits toggle too, typed text joins", () => {
  let s = initState([multi])
  assert.ok(!direct(s.questions) && tabCount(s) === 2)
  s = run(s, { type: "digit", n: 2 }, { type: "digit", n: 2 })
  assert.equal(answerOf(s, 0), null, "ticked twice is off again")
  s = run(s, { type: "digit", n: 2 }, { type: "digit", n: 4 }, { type: "type", text: "Snapchat" }, { type: "enter" })
  assert.ok(onReview(s))
  assert.equal(answerOf(s, 0), "TikTok, Snapchat")
})

test("review: Cancel and Esc decline; Submit with nothing answered stays", () => {
  const two = initState([single, multi])
  const review = run(two, { type: "goto", tab: 2 })
  assert.ok(onReview(review))
  assert.equal(run(review, { type: "enter" }).done, null, "nothing to submit")
  assert.equal(run(review, { type: "digit", n: 2 }).done, "cancel")
  assert.equal(run(review, { type: "move", delta: 1 }, { type: "enter" }).done, "cancel")
  assert.equal(run(two, { type: "esc" }).done, "cancel")
  const answered = run(two, { type: "digit", n: 2 }, { type: "goto", tab: 2 })
  assert.equal(run(answered, { type: "digit", n: 1 }).done, "submit")
  assert.ok(isAnswered(answered, 0) && !isAnswered(answered, 1))
})

test("Chat about this closes the wizard from any question; finished states ignore input", () => {
  const s = initState([single, multi])
  assert.equal(run(s, { type: "row", row: 4 }).done, "chat")
  const over = run(s, { type: "esc" })
  assert.equal(reduce(over, { type: "digit", n: 1 }), over)
})

test("Tab and the arrows switch questions (mirrored in Arabic) and wrap", () => {
  const s = initState([single, multi])
  assert.equal(run(s, { type: "tab", delta: 1 }).tab, 1)
  assert.equal(run(s, { type: "tab", delta: -1 }).tab, 2)
  assert.equal(reduce(initState([single]), { type: "tab", delta: 1 }).tab, 0)
  assert.equal(tabDelta("ArrowRight", false), 1)
  assert.equal(tabDelta("ArrowRight", true), -1)
  assert.equal(tabDelta("ArrowLeft", true), 1)
  assert.equal(tabDelta("x", false), 0)
})
