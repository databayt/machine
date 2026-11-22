// node --test src/components/chat/activity.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { GLYPHS, formatElapsed, formatTokens, glyphFrame, playfulWord, splitNote, statusDetail, summarize } from "./activity-pure.ts"

test("the glyph climbs the six frames and comes back down", () => {
  const seq = Array.from({ length: 12 }, (_, i) => glyphFrame(i))
  assert.deepEqual(seq, ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢", "·", "✢"])
  assert.equal(GLYPHS.length, 6)
  assert.equal(glyphFrame(-1), "✢")
})

test("tokens round to a tenth of a thousand", () => {
  assert.equal(formatTokens(0, false), "0 tokens")
  assert.equal(formatTokens(950, false), "950 tokens")
  assert.equal(formatTokens(1432, false), "1.4k tokens")
  assert.equal(formatTokens(2000, false), "2k tokens")
  assert.equal(formatTokens(12345, false), "12.3k tokens")
  assert.equal(formatTokens(1432, true), "١٫٤ ألف رمز")
  assert.equal(formatTokens(950, true), "٩٥٠ رمز")
})

test("elapsed reads in seconds, then minutes", () => {
  assert.equal(formatElapsed(12, false), "12s")
  assert.equal(formatElapsed(65, false), "1m 5s")
  assert.equal(formatElapsed(12, true), "١٢ث")
  assert.equal(formatElapsed(65, true), "١د ٥ث")
})

test("the detail omits tokens it does not have", () => {
  assert.equal(statusDetail(12, 1432, false, "esc to interrupt"), "12s · ↓ 1.4k tokens · esc to interrupt")
  assert.equal(statusDetail(3, null, false, "esc to interrupt"), "3s · esc to interrupt")
  assert.equal(statusDetail(12, 1432, true, "esc للإيقاف"), "١٢ث · ↓ ١٫٤ ألف رمز · ⁨esc⁩ للإيقاف")
})

test("the old helpers still work", () => {
  assert.equal(playfulWord(["a", "b"], "x", 1) !== "", true)
  assert.deepEqual(summarize(["Read", "Read", "Bash"]), { head: ["Read", "Bash"], rest: 0 })
})

test("a leading Note paragraph splits off the answer", () => {
  assert.deepEqual(splitNote("Note: it moved.\n\nHere you go."), { note: "it moved.", rest: "Here you go." })
  assert.deepEqual(splitNote("Note: only this"), { note: "only this", rest: "" })
  assert.deepEqual(splitNote("Hello\n\nNote: later"), { note: null, rest: "Hello\n\nNote: later" })
})
