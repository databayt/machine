// node --test src/components/tasks/submit.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { percent, progressLine, splitSummary } from "./submit.ts"

test("the composer text is the summary; a next: line is the next person's line", () => {
  assert.deepEqual(splitSummary("  Built the classroom scene.  "), { summary: "Built the classroom scene.", next: null })
  assert.deepEqual(splitSummary("Built it.\nnext: Sara lights it.\nMore detail."), {
    summary: "Built it.\nMore detail.",
    next: "Sara lights it.",
  })
  assert.deepEqual(splitSummary("Done\nNEXT：  review the render"), { summary: "Done", next: "review the render" })
  assert.deepEqual(splitSummary("   "), { summary: "", next: null })
  assert.deepEqual(splitSummary("next: only this"), { summary: "", next: "only this" })
  assert.deepEqual(splitSummary("the next: step is mine"), { summary: "the next: step is mine", next: null })
})

test("percent is whole, capped, and 100 for an empty file", () => {
  assert.equal(percent(0, 10), 0)
  assert.equal(percent(5, 10), 50)
  assert.equal(percent(10, 10), 100)
  assert.equal(percent(11, 10), 100)
  assert.equal(percent(0, 0), 100)
  assert.equal(percent(999, 1000), 99)
})

test("the progress line names the file and the share, in both languages", () => {
  assert.equal(progressLine({ kind: "prepared", count: 1 }, false), "Uploading 1 file…")
  assert.equal(progressLine({ kind: "prepared", count: 3 }, false), "Uploading 3 files…")
  assert.equal(progressLine({ kind: "progress", index: 1, count: 3, name: "a.mp4", sent: 50, total: 200 }, false), "Uploading a.mp4 (2/3) · 25%")
  assert.equal(progressLine({ kind: "progress", index: 0, count: 1, name: "a.mp4", sent: 1, total: 1 }, false), "Uploading a.mp4 · 100%")
  assert.equal(progressLine({ kind: "progress", index: 0, count: 1, name: "a.mp4", sent: 1, total: 4 }, true), "يرفع a.mp4 · 25%")
  assert.equal(progressLine({ kind: "uploaded", index: 0, count: 1, name: "a.mp4", url: "https://x" }, false), "Uploaded a.mp4")
  assert.equal(progressLine({ kind: "posting" }, false), "Sending the receipt…")
})
