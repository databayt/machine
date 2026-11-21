// node --test src/components/chat/fill.test.ts (Node strips the types; no runner dependency).
import assert from "node:assert/strict"
import { test } from "node:test"

import type { Starter } from "../pack/types.ts"
import { renderTemplate, sentenceFor } from "./fill.ts"

const node = (over: Partial<Starter>): Starter => ({
  id: "n",
  label_ar: "ع",
  label_en: "En",
  description_ar: "",
  description_en: "",
  icon: null,
  repos: [],
  set: {},
  children: [],
  source: null,
  then: [],
  prompt: null,
  fill_ar: null,
  fill_en: null,
  ...over,
})

test("every token present", () => {
  assert.equal(renderTemplate("Make {{type}} in {{repo}}", { type: "a reel", repo: "mkan" }), "Make a reel in mkan")
})

test("a missing token drops the separator before it", () => {
  assert.equal(renderTemplate("Video · {{market_label}} · {{type}}", { type: "reel" }), "Video · reel")
  assert.equal(renderTemplate("فيديو، {{market_label}}", {}), "فيديو")
})

test("a missing first token drops the separator after it", () => {
  assert.equal(renderTemplate("{{market_label}} — {{type}} for {{repo}}", { type: "reel", repo: "mkan" }), "reel for mkan")
})

test("an empty value counts as missing; spaces collapse and punctuation hugs", () => {
  assert.equal(renderTemplate("Make  {{a}} for {{repo}} .", { a: "", repo: "" }), "Make for.")
  assert.equal(renderTemplate("Edit {{x}}.", {}), "Edit.")
})

test("the deepest fill wins, else the formula, else nothing", () => {
  const formula = { ar: "ابدأ {{repo}}", en: "Start {{repo}} · {{type_label}}" }
  const a = node({ id: "a", set: { type: "reel" }, label_en: "Reel" })
  const b = node({ id: "b", fill_en: "Cut the {{type_label}} for {{repo}}" })
  assert.equal(sentenceFor([a, b], "mkan", false, formula), "Cut the Reel for mkan")
  assert.equal(sentenceFor([a], "mkan", false, formula), "Start mkan · Reel")
  assert.equal(sentenceFor([node({})], "", false, formula), "Start")
  assert.equal(sentenceFor([a], "mkan", true, formula), "ابدأ mkan")
  assert.equal(sentenceFor([a], "mkan", false, null), "")
  assert.equal(sentenceFor([], "mkan", false, formula), "")
})
