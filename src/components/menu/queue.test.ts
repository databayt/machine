// node --test src/components/menu/queue.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import type { Task } from "../tasks/types.ts"
import { MAX_CARDS, upcoming } from "./queue.ts"

const task = (n: number, over: Partial<Task> = {}): Task => ({
  id: `databayt/hogwarts#${n}`,
  repo: "hogwarts",
  number: n,
  title: `t${n}`,
  url: "",
  department: "tech",
  status: "todo",
  holder: null,
  priority: null,
  sprint: null,
  order: n,
  auto: false,
  kun: { repos: [], items: [], after: [], driver: null },
  afterOpen: [],
  body: "",
  updatedAt: "",
  ...over,
})

test("my orders come right after what I hold, even from another department", () => {
  const order = task(9, { department: "video", assignee: "sedon", film: "databayt/hogwarts#2", formula: "video.order.blender" })
  const view = { mine: [task(1, { holder: "sedon" })], board: [task(3), order, task(4, { holder: "ali" })] }
  assert.deepEqual(upcoming(view, "sedon").map((t) => t.number), [1, 9, 3, 4])
  // For someone else the order is just another card on the board, in queue order.
  assert.deepEqual(upcoming({ mine: [], board: view.board }, "ali").map((t) => t.number), [3, 9, 4])
})

test("an order that arrives in mine is listed once", () => {
  const order = task(9, { assignee: "sedon" })
  assert.deepEqual(upcoming({ mine: [order], board: [order, task(2)] }, "sedon").map((t) => t.number), [9, 2])
})

test("an older server without the new fields still works", () => {
  const old = { ...task(5) } as Partial<Task>
  delete old.assignee
  delete old.film
  delete old.formula
  assert.deepEqual(upcoming({ mine: [], board: [old as Task] }, "sedon").map((t) => t.number), [5])
  assert.deepEqual(upcoming(null, "sedon"), [])
  assert.deepEqual(upcoming({ mine: [], board: [] }, undefined), [])
})

test("at most six cards", () => {
  const board = Array.from({ length: 10 }, (_, i) => task(i + 1))
  assert.equal(upcoming({ mine: [], board }, "x").length, MAX_CARDS)
})
