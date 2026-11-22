// node --test src/components/chat/activity.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { GLYPHS, GLYPH_FRAME_MS, SHIMMER_CYCLE_MS, SHIMMER_STEP_MS, formatElapsed, formatTokens, glyphFrame, playfulWord, splitNote, statusDetail, summarize, turnWord } from "./activity-pure.ts"

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

test("the verb is fixed for the whole turn: one pick from the seed, never a tick", () => {
  const list = ["a", "b", "c", "d", "e", "f"]
  const w = turnWord(list, "3")
  assert.equal(w, playfulWord(list, "3", 0))
  assert.equal(turnWord(list, "3"), w)
  assert.ok(list.includes(w))
  assert.equal(turnWord([], "3"), "")
  // another turn may pick another word
  assert.ok(new Set(["0", "1", "2", "3", "4", "5"].map((s) => turnWord(list, s))).size > 1)
})

test("motion is tuned: 320 ms glyph frames, a 3 s shimmer cycle that a whole verb sweeps inside", () => {
  assert.equal(GLYPH_FRAME_MS, 320)
  assert.equal(SHIMMER_CYCLE_MS, 3000)
  // the longest verb (about 22 characters) is fully swept well before the cycle restarts
  assert.ok(22 * SHIMMER_STEP_MS < SHIMMER_CYCLE_MS / 1.5)
})
