// node --test src/components/chat/block.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { hideBlocks, parsePlan, parseSpec, parseTurn, planFile, specFile, trailingBlock } from "./block-pure.ts"

const fence = (info: string, json: unknown) => "```" + info + "\n" + (typeof json === "string" ? json : JSON.stringify(json)) + "\n```"
const q = { question: "Who is it for?", header: "Audience", multiSelect: false, options: [{ label: "Owners", description: "d" }, { label: "Teachers", description: "d" }] }
const plan = { film: { brand: "Hogwarts", kind: "intro" }, cost: ["Gemini stills: $0", "Voice: free"], stages: { script: { decisions: { structure: "walkthrough" }, tool: "video-agent" }, nonsense: { tool: "x" } } }
const spec = { stage: "visual", decisions: { look: "mixed" }, tool: "blender", notes: "Use the vault palette." }

test("each block is read when it is the last thing in the turn", () => {
  const a = parseTurn(`Two questions.\n\n${fence("kun-questions", { questions: [q] })}\n`)
  assert.equal(a.prose, "Two questions.")
  assert.equal(a.questions?.length, 1)
  assert.ok(!a.plan && !a.spec)
  const b = parseTurn(`The plan.\n\n${fence("kun-plan", plan)}`)
  assert.equal(b.prose, "The plan.")
  assert.deepEqual(b.plan?.cost, ["Gemini stills: $0", "Voice: free"])
  assert.ok(!b.questions && !b.spec)
  const c = parseTurn(`Locking this.\n\n${fence("kun-spec", spec)}\n\n`)
  assert.equal(c.prose, "Locking this.")
  assert.deepEqual(c.spec, { stage: "visual", decisions: { look: "mixed" }, tool: "blender", notes: "Use the vault palette." })
})

test("a block that is not last is ignored, and bad JSON leaves the text exactly as it came", () => {
  const notLast = `${fence("kun-plan", plan)}\n\nOne more thing.`
  assert.deepEqual(parseTurn(notLast), { prose: notLast, questions: null, plan: null, spec: null })
  const bad = "Text\n\n```kun-spec\n{ not json\n```"
  assert.equal(parseTurn(bad).prose, bad)
  assert.equal(trailingBlock(bad, "kun-spec").found, false)
  const twoFences = "```kun-plan\n{\"cost\":[\"a\"]}\n```\nand\n```\ncode\n```"
  assert.equal(trailingBlock(twoFences, "kun-plan").found, false, "the body holds a fence")
  assert.equal(trailingBlock("plain text", "kun-plan").prose, "plain text")
})

test("hideBlocks cuts a streaming text at the last opener of any block", () => {
  assert.equal(hideBlocks("Just prose"), "Just prose")
  assert.equal(hideBlocks('Prose.\n\n```kun-plan\n{ "film": { "a"'), "Prose.")
  assert.equal(hideBlocks('Prose.\n\n```kun-spec\n{"stage":"visual"}\n```'), "Prose.")
  assert.equal(hideBlocks('Prose.\n\n```kun-questions\n{"questions":['), "Prose.")
})

test("parsePlan keeps the film, the cost strings and only the 14 known stages", () => {
  const p = parsePlan({ ...plan, cost: ["a", 3, "", "b"] })!
  assert.deepEqual(Object.keys(p.stages), ["script"])
  assert.deepEqual(p.cost, ["a", "b"])
  assert.deepEqual(p.film, plan.film)
  assert.equal(parsePlan({ stages: { nonsense: {} } }), null)
  assert.equal(parsePlan("x"), null)
  assert.equal(parsePlan([]), null)
})

test("parseSpec needs one of the 14 stages and an object of decisions", () => {
  assert.equal(parseSpec({ stage: "nonsense", decisions: {} }), null)
  assert.equal(parseSpec({ decisions: {} }), null)
  assert.equal(parseSpec({ stage: "visual", decisions: "mixed" }), null)
  assert.deepEqual(parseSpec({ stage: "audio" }), { stage: "audio", decisions: {}, tool: null, notes: null })
  assert.equal(parseSpec({ stage: "audio", tool: 4, notes: 5 })!.tool, null)
})

test("the plan file carries the block's parts and the picks; a spec file is the block's own shape", () => {
  assert.deepEqual(planFile(null, { kind: "intro" }), { picks: { kind: "intro" } })
  const f = planFile(parsePlan(plan), { kind: "intro" })
  assert.deepEqual(Object.keys(f).sort(), ["cost", "film", "picks", "stages"])
  assert.deepEqual(specFile(parseSpec(spec)!), spec)
  assert.deepEqual(specFile(parseSpec({ stage: "audio" })!), { stage: "audio", decisions: {} })
})
