// node --test src/components/chat/clamp.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { CLAMP_LINES, LINE_CHARS, clampLines, startsFolded } from "./clamp-pure.ts"

test("three short lines are not folded, four are", () => {
  assert.deepEqual(clampLines("a\nb\nc"), { clamped: false, lines: 3 })
  assert.deepEqual(clampLines("a\nb\nc\nd"), { clamped: true, lines: 4 })
  assert.equal(CLAMP_LINES, 3)
})

test("a long line counts as it wraps", () => {
  assert.equal(clampLines("x".repeat(LINE_CHARS)).lines, 1)
  assert.equal(clampLines("x".repeat(LINE_CHARS + 1)).lines, 2)
  assert.equal(clampLines("x".repeat(LINE_CHARS * 3)).clamped, false)
  assert.equal(clampLines("x".repeat(LINE_CHARS * 3 + 1)).clamped, true)
})

test("blank lines count, trailing space does not, empty is nothing", () => {
  assert.deepEqual(clampLines("a\n\nb\n\nc"), { clamped: true, lines: 5 })
  assert.deepEqual(clampLines("a\nb\n\n\n"), { clamped: false, lines: 2 })
  assert.deepEqual(clampLines("   \n "), { clamped: false, lines: 0 })
  assert.deepEqual(clampLines(""), { clamped: false, lines: 0 })
})

test("Arabic and emoji count by characters, not code units", () => {
  assert.equal(clampLines("م".repeat(LINE_CHARS)).lines, 1)
  assert.equal(clampLines("😀".repeat(LINE_CHARS)).lines, 1)
})

test("the max and the width can be changed", () => {
  assert.equal(clampLines("a\nb", 1).clamped, true)
  assert.equal(clampLines("abcdef", 3, 2).lines, 3)
})

test("only the first message of a session starts folded", () => {
  assert.equal(startsFolded(0), true)
  assert.equal(startsFolded(2), false)
})
