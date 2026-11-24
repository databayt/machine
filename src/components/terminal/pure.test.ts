import assert from "node:assert/strict"
import { test } from "node:test"

import { freshPart } from "./pure.ts"

const b = (s: string) => new TextEncoder().encode(s)
const t = (u: Uint8Array) => new TextDecoder().decode(u)

test("a chunk after the replay is written whole", () => {
  const r = freshPart({ end: 11, data: "" }, b("world"), 6)
  assert.equal(t(r!.bytes), "world")
  assert.equal(r!.seen, 11)
})

test("a chunk the replay already covered is skipped", () => {
  assert.equal(freshPart({ end: 6, data: "" }, b("hello "), 6), null)
  assert.equal(freshPart({ end: 3, data: "" }, b("abc"), 6), null)
})

test("a chunk that straddles the replay is cut to its new tail", () => {
  const r = freshPart({ end: 11, data: "" }, b("lo world"), 6)
  assert.equal(t(r!.bytes), "world")
  assert.equal(r!.seen, 11)
})
