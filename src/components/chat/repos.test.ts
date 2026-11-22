// node --test src/components/chat/repos.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { filterRepos, pickRepo, repoLabel } from "./repos.ts"

const menu = [
  { name: "hogwarts", word: null },
  { name: "kun", word: null },
  { name: "marketing", word: "landing" },
]

test("a repo with a word shows the word; its name stays the value", () => {
  assert.equal(repoLabel(menu[2]!), "landing")
  assert.equal(repoLabel(menu[0]!), "hogwarts")
})

test("the saved repo stays while it is on the menu, else hogwarts (the first) leads", () => {
  assert.equal(pickRepo("kun", menu), "kun")
  assert.equal(pickRepo("mkan", menu), "hogwarts")
  assert.equal(pickRepo(undefined, menu), "hogwarts")
  assert.equal(pickRepo("kun", []), "")
})

test("search matches the name or the word", () => {
  assert.deepEqual(filterRepos(menu, "hogwarts", "land").map((e) => e.name), ["marketing"])
  assert.deepEqual(filterRepos(menu, "hogwarts", "MARK").map((e) => e.name), ["marketing"])
  assert.deepEqual(filterRepos(menu, "kun", "").map((e) => e.name), ["hogwarts", "marketing"])
})
