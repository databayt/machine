// node --test src/components/chat/refine.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { parseRefined } from "./refine.ts"

const ids = ["plan", "do", "workflow", "quick"]

test("strips the route line and keeps a known route", () => {
  assert.deepEqual(parseRefined("Plan an intro film.\nAudience: owners\n\nroute: plan", ids), {
    text: "Plan an intro film.\nAudience: owners",
    route: "plan",
  })
})

test("an unknown route is dropped, the line still stripped", () => {
  assert.deepEqual(parseRefined("Brief\nroute: elsewhere", ids), { text: "Brief", route: null })
})

test("no route line leaves the text whole", () => {
  assert.deepEqual(parseRefined("  خطّط فيلماً تعريفياً.  ", ids), { text: "خطّط فيلماً تعريفياً.", route: null })
})

test("unwraps a code fence, route outside or inside", () => {
  assert.deepEqual(parseRefined("```\nBrief\n```\nroute: `workflow`", ids), { text: "Brief", route: "workflow" })
  assert.deepEqual(parseRefined("```\nBrief\nroute: quick\n```", ids), { text: "Brief", route: "quick" })
})

test("a route word mid-text is not taken", () => {
  assert.deepEqual(parseRefined("route: plan\nThen the brief.", ids), { text: "route: plan\nThen the brief.", route: null })
})
