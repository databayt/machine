// node --test src/components/chat/keywords.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { type SkillItem, type SlashEntry, allSpans, chipFacts, firstOpenSlot, matchSkills, shouldProbe, slashContext, slashFilter } from "./keywords.ts"

const item = (name: string, keywords: string[], extra: Partial<SkillItem> = {}): SkillItem => ({
  id: `skill:${name}`,
  name,
  title: name,
  description: "",
  keywords,
  dependencies: [],
  ...extra,
})

// Keywords taken from the real SKILL.md "Triggers on" lists.
const items: SkillItem[] = [
  { ...item("video", []), id: "pack:video", dependencies: ["skill:produce", "skill:edit"] },
  item("produce", ["produce", "video production", "make a video for hogwarts", "إنتاج فيديو"]),
  item("edit", ["edit video", "rough cut", "قص الفيديو", "مونتاج"]),
  item("higgs", ["generate video", "nano banana"]),
  item("probe", ["profile"]),
  item("process", ["pro tip"]),
]

test("a phrase from the department's kit finds the skill", () => {
  const m = matchSkills("make a video for hogwarts", items, "video")
  assert.equal(m[0]?.name, "produce")
  assert.deepEqual(m[0]?.spans, [[0, 25]])
})

test("Arabic is folded: the article and diacritics do not matter", () => {
  assert.equal(matchSkills("قص الفيديو", items, "video")[0]?.name, "edit")
})

test("diacritics are stripped", () => {
  assert.equal(matchSkills("قَصّ الفيديو", items, "video")[0]?.name, "edit")
  assert.equal(matchSkills("إنتاج الفيديو", items, "video")[0]?.name, "produce")
  assert.equal(matchSkills("إنتاج فيديو للمدرسة", items, "video")[0]?.name, "produce")
})

test("hidden skills never match", () => {
  assert.equal(matchSkills("قص الفيديو", items, "video", ["skill:edit"]).length, 0)
  assert.equal(matchSkills("rough cut now", items, "video", ["edit"]).length, 0)
})

test("a department's kit outranks other items; a lone foreign word is not enough", () => {
  assert.equal(matchSkills("generate video", items, "video").length, 1)
  assert.equal(matchSkills("edit video", items, "video")[0]?.name, "edit")
  assert.deepEqual(allSpans(matchSkills("please rough cut it", items, "video")), [[7, 16]])
  assert.equal(matchSkills("profile", items, "video").length, 0)
})

test("the probe needs three words and no slash", () => {
  assert.equal(shouldProbe("make a video"), true)
  assert.equal(shouldProbe("make video"), false)
  assert.equal(shouldProbe("/pro a b c"), false)
  assert.equal(shouldProbe("   "), false)
  assert.equal(shouldProbe("اصنع لي فيديو"), true)
})

test("the slash context is a fragment after whitespace, not a path", () => {
  assert.deepEqual(slashContext("/pro", 4), { start: 0, fragment: "pro" })
  assert.deepEqual(slashContext("do it /ed", 9), { start: 6, fragment: "ed" })
  assert.deepEqual(slashContext("a /", 3), { start: 2, fragment: "" })
  assert.equal(slashContext("a/b", 3), null)
  assert.equal(slashContext("open /Users/x", 13), null)
  assert.equal(slashContext("/pro more", 9), null)
})

const entry = (kind: SlashEntry["kind"], name: string, group: SlashEntry["group"], extra: Partial<SlashEntry> = {}): SlashEntry => ({
  kind,
  id: name,
  name,
  title: name,
  hint: "",
  keywords: [],
  group,
  ...extra,
})

test("/pro ranks the name prefix first, then substrings, by group", () => {
  const entries = [
    entry("formula", "video.film.plan", 0, { hint: "a promo film" }),
    entry("skill", "edit", 1, { keywords: ["improve"] }),
    entry("skill", "produce", 1),
    entry("skill", "profile", 2),
    entry("skill", "approve", 2),
  ]
  assert.deepEqual(slashFilter("pro", entries).map((e) => e.name), ["produce", "profile", "video.film.plan", "edit", "approve"])
  assert.deepEqual(slashFilter("", entries).map((e) => e.name), ["video.film.plan", "edit", "produce", "profile", "approve"])
  assert.deepEqual(slashFilter("zzz", entries), [])
})

test("chip facts are at most three short values; the first open slot is found", () => {
  assert.deepEqual(chipFacts({ a: "x", b: " ", c: "y", d: "z", e: "w" }), ["x", "y", "z"])
  assert.equal(chipFacts({ a: "q".repeat(40) })[0]?.length, 28)
  assert.deepEqual(chipFacts(null), [])
  assert.deepEqual(firstOpenSlot("a [unfilled: scene] b [unfilled: to]"), [2, 19])
  assert.equal(firstOpenSlot("all filled"), null)
})
