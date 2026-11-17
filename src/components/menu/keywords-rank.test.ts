// node --test (Node strips the types; no runner dependency).
import assert from "node:assert/strict"
import { test } from "node:test"

import type { CatalogItem } from "../catalog/types.ts"
import { rankKeywords } from "./keywords-rank.ts"

const item = (id: string, keywords: string[], dependencies: string[] = []) =>
  ({ id, keywords, dependencies }) as unknown as CatalogItem

test("keywords that run through more of the department's kit rank first", () => {
  const catalog = [
    item("pack:sales", ["sales"], ["skill:scrape", "skill:funnel", "agent:lead"]),
    item("skill:scrape", ["leads", "scrape"]),
    item("skill:funnel", ["leads", "follow up"]),
    item("agent:lead", ["leads", "prospect"]),
    item("skill:edit", ["reel"]),
  ]
  const words = rankKeywords(catalog, "sales").map((k) => k.word)
  assert.equal(words[0], "leads")
  assert.ok(!words.includes("reel"), "items outside the pack never show")
})

test("no pack, no cloud", () => {
  assert.deepEqual(rankKeywords([item("skill:x", ["x"])], "sales"), [])
})
