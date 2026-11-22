// node --test src/components/chat/model.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { FAMILIES, effectiveModel, familyOf, modelName } from "./model-pure.ts"

test("a model string maps to its family and name", () => {
  assert.equal(modelName("opus"), "Opus 5.5")
  assert.equal(modelName("claude-sonnet-5-5"), "Sonnet 5.5")
  assert.equal(modelName("haiku"), "Haiku 5.5")
  assert.equal(modelName(null), "Claude")
})

test("Fable is never a family, never offered", () => {
  assert.equal(familyOf("fable"), null)
  assert.equal(familyOf("claude-fable-1"), null)
  assert.deepEqual([...FAMILIES], ["opus", "sonnet", "haiku"])
})

test("the pick beats the formula, the formula beats the route", () => {
  assert.equal(effectiveModel("haiku", "sonnet", "opus"), "haiku")
  assert.equal(effectiveModel(null, "sonnet", "opus"), "sonnet")
  assert.equal(effectiveModel(null, null, "opus"), "opus")
  assert.equal(effectiveModel(null, "fable", "opus"), "opus")
})
