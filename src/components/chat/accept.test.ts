// node --test src/components/chat/accept.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { ACCEPT_FILM, ACCEPT_ISSUE, ACCEPT_STEP, acceptMessage, showAccept, turnRoute, workingRoute } from "./accept.ts"

test("a film plan creates the production, its film issue and a work order per piece handed over", () => {
  const m = acceptMessage("film")
  assert.equal(m, ACCEPT_FILM)
  for (const part of ["`produce new`", "`produce issue <ID> --create`", "`produce order <ID> <formula> --to <login> --create`", "stop at the next gate"])
    assert.ok(m.includes(part), part)
})

test("a work order becomes one issue", () => {
  assert.equal(acceptMessage("issue"), ACCEPT_ISSUE)
  assert.ok(ACCEPT_ISSUE.includes("Create the work order as an issue"))
})

test("any other plan carries out the first step and stops at the next gate", () => {
  const want = "The plan is accepted. Carry out the first step, then stop at the next gate."
  for (const record of ["none", "", null, undefined, "other"]) assert.equal(acceptMessage(record), want)
  assert.equal(ACCEPT_STEP, want)
})

test("the working route is the first one that is not plan mode", () => {
  const routes = [
    { id: "plan", mode: "plan" },
    { id: "do", mode: "auto" },
    { id: "quick", mode: "auto" },
  ]
  assert.equal(workingRoute(routes)?.id, "do")
  assert.equal(workingRoute([{ id: "plan", mode: "plan" }]), null)
  assert.equal(workingRoute([]), null)
})

test("a turn runs on the route asked for, else the one chosen", () => {
  assert.equal(turnRoute("do", "plan"), "do")
  assert.equal(turnRoute(null, "plan"), "plan")
  assert.equal(turnRoute(undefined, null), null)
})

test("Accept shows only under the last finished plan turn, and only when there is somewhere to work", () => {
  const ok = { ranInPlan: true, last: true, busy: false, streaming: false, hasText: true, failed: false, hasWorkingRoute: true }
  assert.equal(showAccept(ok), true)
  for (const off of [
    { ranInPlan: false },
    { last: false },
    { busy: true },
    { streaming: true },
    { hasText: false },
    { failed: true },
    { hasWorkingRoute: false },
  ])
    assert.equal(showAccept({ ...ok, ...off }), false, JSON.stringify(off))
})
