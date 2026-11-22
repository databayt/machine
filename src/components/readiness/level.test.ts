import assert from "node:assert/strict"
import { test } from "node:test"

import { levelWords, needsAttention } from "./level.ts"

const gate = (level: "green" | "orange" | "red" | "unknown", core: string[] = [], extra: string[] = []) => ({
  level,
  coreFailing: core,
  extraFailing: extra,
})

test("each level has its words, in both languages, with the right count", () => {
  assert.equal(levelWords(gate("green"), false), "Ready to contribute")
  assert.equal(levelWords(gate("orange", [], ["a", "b", "c"]), false), "Can contribute — 3 extras missing")
  assert.equal(levelWords(gate("orange", [], ["a"]), false), "Can contribute — 1 extra missing")
  assert.equal(levelWords(gate("red", ["x", "y"], ["a"]), false), "Contribution blocked — 2 core steps missing")
  assert.equal(levelWords(gate("red", ["x"]), false), "Contribution blocked — 1 core step missing")
  assert.equal(levelWords(gate("unknown"), false), "Checking…")
  assert.match(levelWords(gate("red", ["x", "y"]), true), /2/)
  assert.equal(levelWords(gate("green"), true), "جاهز للمساهمة")
})

test("only orange and red ask for attention", () => {
  assert.deepEqual((["green", "orange", "red", "unknown"] as const).map(needsAttention), [false, true, true, false])
})
