// node --test src/components/chat/stage.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { parseStatus } from "../production/pure.ts"
import { parseStage, splitAnswers, stageFromStatus, stageHeading, stageSessionKey } from "./stage-pure.ts"

// The contract's example of `station.mjs stage visual --json`.
const RAW = {
  film: "hogwarts-intro-introduction-v001",
  stage: "visual",
  n: 7,
  group: "make",
  label: { en: "Visual", ar: "Visual" },
  ask: "agent",
  gate: "look",
  done: false,
  locked: true,
  lock: { by: "abdout", at: "2026-11-23T10:00:00.000Z", sha: "x", source: "plan" },
  current: { look: "mixed", threed: "blender" },
  tool: "blender",
  tool_label: "Blender on this Mac (blender.sh, MCP blender / blender-lab)",
  lane: "build",
  summary: ["Look: Real screens + 3D", "3D: Blender on this Mac", "Output: 06-production/visual-manifest.json"],
  questions: [
    { key: "look", question: "Which look?", header: "Look", multiSelect: false, options: [{ value: "mixed", label: "Real screens + 3D", description: "Real product captures inside 3D scenes." }, { value: "captures", label: "Real screens", description: "" }] },
  ],
}

test("parseStage reads the station's stage JSON", () => {
  const v = parseStage(RAW)!
  assert.equal(v.film, "hogwarts-intro-introduction-v001")
  assert.equal(v.stage, "visual")
  assert.equal(v.n, 7)
  assert.equal(v.label, "Visual")
  assert.equal(v.ask, "agent")
  assert.equal(v.gate, "look")
  assert.ok(v.locked && !v.done)
  assert.equal(v.lockBy, "abdout")
  assert.equal(v.summary.length, 3)
  assert.equal(v.toolLabel, RAW.tool_label)
  assert.equal(v.questions.length, 1)
  assert.equal(v.questions[0]!.key, "look")
  assert.equal(v.questions[0]!.options[1]!.value, "captures")
  assert.ok(!v.fallback)
})

test("parseStage is tolerant: an error answer or a missing film is nothing, the rest has defaults", () => {
  assert.equal(parseStage({ error: "unknown stage" }), null)
  assert.equal(parseStage(null), null)
  const v = parseStage({ film: "f", stage: "qa" })!
  assert.deepEqual([v.n, v.label, v.ask, v.locked, v.summary, v.questions], [0, "qa", "script", false, [], []])
  // a malformed question is dropped, not fatal
  assert.deepEqual(parseStage({ film: "f", stage: "qa", questions: [{ key: "x" }] })!.questions, [])
})

test("splitAnswers: picked values by key, and the text typed on Type something", () => {
  assert.deepEqual(splitAnswers([{ header: "Look", question: "?", answer: "Real screens", key: "look", value: "captures" }, { header: "3D", question: "?", answer: "Blender", key: "threed", value: "blender" }]), { set: { look: "captures", threed: "blender" }, typed: null })
  assert.deepEqual(splitAnswers([{ header: "Look", question: "?", answer: "darker please", key: "look", typed: true }]), { set: {}, typed: "darker please" })
  assert.deepEqual(splitAnswers([{ header: "Q", question: "?", answer: "no key" }]), { set: {}, typed: null })
})

test("stageFromStatus builds a view from the status alone, with no questions", () => {
  const film = parseStatus({ id: "f-1", stages: { request: true, script: true, visual: false }, gates: { look: null }, locks: { script: { by: "a" } } })!
  const v = stageFromStatus(film, "visual")!
  assert.equal(v.n, 7)
  assert.equal(v.label, "Visual")
  assert.equal(v.gate, "look")
  assert.ok(v.fallback && v.questions.length === 0 && !v.locked)
  assert.equal(v.ask, "agent")
  assert.equal(stageFromStatus(film, "script")!.locked, true)
  assert.equal(stageFromStatus(film, "nope"), null)
})

test("a stage's chat is remembered under film + stage, and the heading names the lock", () => {
  assert.equal(stageSessionKey("f-1", "visual"), "kun.video.stage-session.f-1.visual")
  const words = { locked: "locked", open: "open", by: "by" }
  assert.equal(stageHeading(parseStage(RAW)!, words), "07 visual · locked by abdout")
  assert.equal(stageHeading({ ...parseStage(RAW)!, locked: false }, words), "07 visual · open")
})
