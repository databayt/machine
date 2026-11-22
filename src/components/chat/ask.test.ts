// node --test src/components/chat/ask.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { ANSWERS_SUFFIX, DECLINED_MESSAGE, answersMessage, hideBlock, isAnswersMessage, isDeclinedMessage, parseAnswers, parseAsk } from "./ask-pure.ts"

const block = (json: unknown) => "```kun-questions\n" + (typeof json === "string" ? json : JSON.stringify(json)) + "\n```"
const opt = (label: string) => ({ label, description: `${label} desc` })
const q1 = { question: "Who is it for?", header: "Audience", multiSelect: false, options: [opt("Owners"), opt("Teachers")] }

test("a valid trailing block gives the prose and the questions", () => {
  const r = parseAsk(`Two quick questions.\n\n${block({ questions: [q1] })}\n`)
  assert.equal(r.prose, "Two quick questions.")
  assert.deepEqual(r.questions, [q1])
})

test("a block that is not the last thing is ignored", () => {
  const text = `${block({ questions: [q1] })}\n\nAnd then more prose.`
  assert.deepEqual(parseAsk(text), { prose: text, questions: null })
  assert.equal(parseAsk("no block here").questions, null)
})

test("invalid JSON keeps the whole text", () => {
  const text = "Hmm.\n\n" + block("{ nope")
  assert.deepEqual(parseAsk(text), { prose: text, questions: null })
})

test("a header over 12 characters is cut, a missing one comes from the question", () => {
  const long = { ...q1, header: "Audience of the film" }
  assert.equal(parseAsk(block({ questions: [long] })).questions![0]!.header, "Audience of")
  const none = { question: "Market first?", options: q1.options }
  assert.equal(parseAsk(block({ questions: [none] })).questions![0]!.header, "Market")
})

test("invalid questions are dropped, the valid ones stay", () => {
  const oneOption = { ...q1, options: [opt("Only")] }
  const fiveOptions = { ...q1, options: ["a", "b", "c", "d", "e"].map(opt) }
  const noLabel = { ...q1, options: [{ description: "x" }, opt("b")] }
  const noText = { ...q1, question: "  " }
  const r = parseAsk(block({ questions: [oneOption, fiveOptions, noLabel, noText, q1, "x", null] }))
  assert.deepEqual(r.questions, [q1])
  assert.equal(parseAsk(block({ questions: [oneOption] })).questions, null)
  assert.equal(parseAsk(block({ questions: [oneOption] })).prose, "")
  assert.equal(parseAsk(block({ other: 1 })).questions, null)
})

test("at most four questions; multiSelect is a real boolean", () => {
  const many = Array.from({ length: 6 }, (_, i) => ({ ...q1, question: `Q${i}?`, multiSelect: i === 1 ? true : "yes" }))
  const r = parseAsk(block({ questions: many })).questions!
  assert.equal(r.length, 4)
  assert.deepEqual(r.map((x) => x.multiSelect), [false, true, false, false])
})

test("a block still being written is hidden while the turn streams", () => {
  assert.equal(hideBlock("One line.\n\n```kun-questions\n{\"quest"), "One line.")
  assert.equal(hideBlock("Plain text."), "Plain text.")
})

test("the answers message is Claude Code's wording; multi-select answers are joined", () => {
  const m = answersMessage([
    { header: "Audience", question: "Who is it for?", answer: "Owners" },
    { header: "Length", question: 'How "long"?', answer: "30 s, 60 s" },
  ])
  assert.equal(
    m,
    `User has answered your questions: "Who is it for?"="Owners", "How 'long'?"="30 s, 60 s". ${ANSWERS_SUFFIX}`,
  )
  assert.ok(isAnswersMessage(m))
  assert.deepEqual(parseAnswers(m).map((a) => a.answer), ["Owners", "30 s, 60 s"])
  assert.ok(isDeclinedMessage(DECLINED_MESSAGE) && !isDeclinedMessage(m) && !isAnswersMessage(DECLINED_MESSAGE))
})

test("Arabic answers travel as they are", () => {
  const m = answersMessage([{ header: "الجمهور", question: "لمن الفيلم؟", answer: "أولياء الأمور" }])
  assert.ok(m.includes('"لمن الفيلم؟"="أولياء الأمور"'))
})
