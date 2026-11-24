// node --test src/components/web/handoff.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { handoffPrompt, MAX_TRANSCRIPT_CHARS, transcriptText } from "./handoff.ts"

const turns = [
  { role: "user", text: " What should we call it? " },
  { role: "assistant", text: "Kun Seats." },
  { role: "assistant", text: "   " },
]

test("a transcript names who spoke, oldest first, and drops empty turns", () => {
  assert.equal(transcriptText(turns, "ChatGPT"), "You: What should we call it?\n\nChatGPT: Kun Seats.")
  assert.equal(transcriptText([], "ChatGPT"), "")
})

test("a long transcript loses its oldest turns, whole", () => {
  const many = Array.from({ length: 40 }, (_, i) => ({ role: i % 2 ? "assistant" : "user", text: `turn ${i} ${"x".repeat(60)}` }))
  const out = transcriptText(many, "GLM", 500)
  assert.ok(out.startsWith("[earlier turns trimmed]\n\n"))
  assert.ok(out.length <= 500 + 30)
  assert.ok(out.includes("turn 39") && !out.includes("turn 0 "))
  assert.ok(MAX_TRANSCRIPT_CHARS > 0)
})

test("each next step is a prompt that carries the transcript", () => {
  const t = transcriptText(turns, "Gemini")
  const idea = handoffPrompt("idea", "Gemini", t)
  assert.ok(idea.includes("idea skill") && idea.includes("GitHub issue") && idea.endsWith(`---\n${t}\n---`))
  const spec = handoffPrompt("spec", "Gemini", t, "databayt/kun#12")
  assert.ok(spec.includes("databayt/kun#12") && spec.includes("gh issue comment") && spec.includes(t))
  const chat = handoffPrompt("chat", "Gemini", t)
  assert.ok(chat.startsWith("Context from my brainstorm with Gemini") && chat.includes(t))
})
