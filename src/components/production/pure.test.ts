// node --test src/components/production/pure.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { collapsed, fillSlots, gateOf, collapsedText, defaultFilm, fileKind, filesFor, fontName, orderFormula, parseBrand, parseList, parseScenes, parseStatus, sceneKind, slotsFor, slugOf, stageFormula, withSource } from "./pure.ts"

// What `produce.mjs status <id> --json` prints today: a stage map of booleans, gates as null or { by, at }.
const TODAY = {
  id: "hogwarts-intro-introduction-v001",
  title: "hogwarts intro — introduction",
  status: "in-production",
  version: "V001",
  stages: { request: true, research: true, brief: true, script: true, storyboard: true, assets: true, visual: false, audio: false, edit: false, render: false, qa: false, approval: false, cdn: false, publish: false },
  gates: { script: { by: "abdout", at: "2026-11-21T15:26:18Z" }, look: null, version: null, publish: null },
}

test("today's status has 14 stages, six done, and 07 visual next", () => {
  const f = parseStatus(TODAY)!
  assert.equal(f.stages.length, 14)
  assert.equal(f.stages.filter((s) => s.done).length, 6)
  assert.equal(f.next?.name, "visual")
  assert.equal(f.next?.n, 7)
  assert.equal(f.slug, "intro-introduction")
  assert.deepEqual(f.gates.map((g) => [g.name, g.signed]), [["script", true], ["look", false], ["version", false], ["publish", false]])
})

test("the collapsed line reads like the plan's", () => {
  const f = parseStatus(TODAY)!
  const c = collapsed(f)
  assert.equal(c.next, "07 visual")
  assert.equal(c.dots.filter(Boolean).length, 6)
  assert.equal(collapsedText(f), "▸ intro-introduction · ●●●●●●○○○○○○○○ 07 visual · script ✔ look ○ version ○ publish ○")
})

test("stage cells may be objects with a status, or a list", () => {
  const a = parseStatus({ id: "x-a-b-v001", stages: { request: { status: "done", file: "01/r.json", at: "t" }, research: { status: "todo" } } })!
  assert.equal(a.stages[0]!.done, true)
  assert.equal(a.stages[0]!.file, "01/r.json")
  assert.equal(a.stages[1]!.done, false)
  const b = parseStatus({ id: "x-a-b-v001", stages: [{ name: "request", done: true }, { stage: "brief", status: "approved" }], next: "research" })!
  assert.deepEqual(b.stages.filter((s) => s.done).map((s) => s.name), ["request", "brief"])
  assert.equal(b.next?.name, "research")
})

test("a variant grid counts per stage", () => {
  const f = parseStatus({ ...TODAY, variants: ["sd-ar", "sd-en"], variantGrid: { "sd-ar": { audio: "done", edit: "todo" }, "sd-en": { audio: "todo", edit: "todo" } }, focus: ["sd-ar"] })!
  assert.deepEqual(f.variants, ["sd-ar", "sd-en"])
  assert.deepEqual(f.stages.find((s) => s.name === "audio")!.variants, { done: 1, total: 2 })
  assert.equal(f.stages.find((s) => s.name === "request")!.variants, null)
  assert.equal(f.lang, "ar")
})

test("junk is skipped, never thrown", () => {
  assert.equal(parseStatus(null), null)
  assert.equal(parseStatus({ stages: {} }), null)
  assert.equal(parseStatus("text"), null)
  assert.deepEqual(parseList("nope"), [])
  assert.deepEqual(parseList([1, null, { title: "no id" }]), [])
  const f = parseStatus({ id: "a-b-c-v001" })!
  assert.equal(f.stages.length, 14)
  assert.equal(f.next?.name, "request")
})

test("the list orders newest first and the default is the latest, or the kept one", () => {
  const films = parseList([
    { id: "hogwarts-intro-a-v001", category: "intro", subject: "a", createdAt: "2026-11-01T00:00:00Z", brand: "hogwarts" },
    { id: "hogwarts-promo-b-v001", category: "promo", subject: "b", createdAt: "2026-11-20T00:00:00Z", updatedAt: "2026-11-22T00:00:00Z", brand: "hogwarts" },
  ])
  assert.equal(films[0]!.id, "hogwarts-promo-b-v001")
  assert.equal(films[0]!.slug, "promo-b")
  assert.equal(defaultFilm(films), "hogwarts-promo-b-v001")
  assert.equal(defaultFilm(films, "hogwarts-intro-a-v001"), "hogwarts-intro-a-v001")
  assert.equal(defaultFilm(films, "gone"), "hogwarts-promo-b-v001")
  assert.equal(defaultFilm([]), null)
})

test("slugs drop the brand and the version", () => {
  assert.equal(slugOf("hogwarts-intro-introduction-v001", "hogwarts"), "intro-introduction")
  assert.equal(slugOf("mkan-reel-homes-v002"), "reel-homes")
})

const SHOTS = {
  scenes: [
    { id: "s01-hook", durationS: 3, narration: { ar: "المدرسة كلها", en: "Your whole school" }, visual: "phone", source: { type: "capture", ref: "x" }, flow: "feature-dashboard" },
    { id: "s02-problem", durationS: 5, narration: { en: "Ledgers" }, source: { type: "graphic" } },
    { id: "s03-world", source: { type: "3d" } },
  ],
}
const MANIFEST = { scenes: { "s01-hook": { ar: ["/m/a-ar.png"], en: ["/m/a-en.png"] }, "s03-world": ["/m/w.glb", { path: "/m/w.mp4" }] } }

test("scenes join the shot list to the manifest", () => {
  const s = parseScenes(SHOTS, MANIFEST, "en")
  assert.deepEqual(s.map((x) => [x.id, x.kind, x.files.length]), [["s01-hook", "capture", 2], ["s02-problem", "graphic", 0], ["s03-world", "3d", 2]])
  assert.equal(s[0]!.line, "Your whole school")
  assert.equal(s[0]!.flow, "feature-dashboard")
  assert.equal(s[0]!.seconds, 3)
  assert.equal(parseScenes(SHOTS, MANIFEST, "ar")[1]!.line, "Ledgers", "falls back to the other language")
  assert.deepEqual(s[2]!.files.map((f) => f.kind), ["3d", "video"])
  assert.deepEqual(parseScenes(null, MANIFEST), [])
  assert.equal(parseScenes(SHOTS, null).length, 3)
})

test("a manifest as a list of scenes works too", () => {
  const s = parseScenes(SHOTS, { scenes: [{ scene: "s01-hook", files: ["/m/x.png"] }, { id: "s03-world", assets: [{ file: "/m/y.png" }] }] })
  assert.equal(s[0]!.files[0]!.path, "/m/x.png")
  assert.equal(s[2]!.files[0]!.path, "/m/y.png")
})

test("files for a language keep the untagged and fall back to all", () => {
  const sc = parseScenes(SHOTS, MANIFEST)[0]!
  assert.deepEqual(filesFor(sc, "ar").map((f) => f.path), ["/m/a-ar.png"])
  assert.deepEqual(filesFor(sc, "fr").map((f) => f.path), ["/m/a-ar.png", "/m/a-en.png"])
  assert.equal(filesFor(sc, null).length, 2)
})

test("kinds come from the type words and the file endings", () => {
  assert.equal(sceneKind("capture"), "capture")
  assert.equal(sceneKind("screen-recording"), "capture")
  assert.equal(sceneKind("blender"), "3d")
  assert.equal(sceneKind("generated-image"), "image")
  assert.equal(sceneKind("generated-video"), "video")
  assert.equal(sceneKind("hyperframes"), "graphic")
  assert.equal(sceneKind(undefined), "other")
  assert.equal(fileKind("/a/b.PNG"), "image")
  assert.equal(fileKind("/a/b.mp4"), "video")
  assert.equal(fileKind("/a/b.glb"), "3d")
  assert.equal(fileKind("/a/b.wav"), "other")
})

test("a click chooses the formula and the slots", () => {
  assert.equal(stageFormula("visual"), "video.stage.visual")
  assert.equal(orderFormula("capture"), "video.order.capture")
  assert.equal(orderFormula("3d"), "video.order.blender")
  assert.equal(orderFormula("image"), "video.order.image")
  assert.equal(orderFormula("video"), "video.order.filmcraft")
  assert.equal(orderFormula("graphic"), "video.stage.visual")
  assert.deepEqual(slotsFor("f"), { project: "f" })
  assert.deepEqual(slotsFor("f", { id: "s1", flow: "x" }), { project: "f", scene: "s1", flow: "x" })
  assert.deepEqual(slotsFor("f", { id: "s1", flow: null }), { project: "f", scene: "s1" })
  assert.deepEqual(withSource({ project: "f" }, "/v/a.mp4"), { project: "f", source: "/v/a.mp4" })
  assert.deepEqual(withSource({ project: "f" }, null), { project: "f" })
})

test("the brand reads fonts, colours and voice", () => {
  const b = parseBrand({ identity: "balqalam", logoData: "data:image/png;base64,AA==", fonts: { arabicBody: "thmanyah-sans-500.ttf", latin: "Geist" }, palette: { canvas: "#faf9f5", ink: "#141413", numerals: "arab" }, voice: "house-ar" })!
  assert.equal(b.logo, "data:image/png;base64,AA==")
  assert.deepEqual(b.fonts, [{ role: "arabicBody", name: "Thmanyah Sans 500" }, { role: "latin", name: "Geist" }])
  assert.deepEqual(b.palette.map((p) => p.hex), ["#faf9f5", "#141413"])
  assert.equal(b.voice, "house-ar")
  assert.equal(parseBrand({}), null)
  assert.equal(fontName("Geist"), "Geist")
})

test("a gate sits on the stage it signs off", () => {
  const f = parseStatus(TODAY)!
  assert.equal(gateOf(f, "script")?.signed, true)
  assert.equal(gateOf(f, "visual")?.name, "look")
  assert.equal(gateOf(f, "visual")?.signed, false)
  assert.equal(gateOf(f, "approval")?.name, "version")
  assert.equal(gateOf(f, "render"), null)
})

test("a fill gets the film for film formulas only, and the attached video when there is one", () => {
  const known = { project: "f-1", source: "/v/promo.mp4" }
  assert.deepEqual(fillSlots("video.stage.visual", {}, known), { project: "f-1", source: "/v/promo.mp4" })
  assert.deepEqual(fillSlots("video.order.capture", { scene: "s1" }, known), { scene: "s1", project: "f-1", source: "/v/promo.mp4" })
  assert.deepEqual(fillSlots("video.film.continue", {}, { project: "f-1" }), { project: "f-1" })
  // a new film is not pinned to the open one
  assert.deepEqual(fillSlots("video.film.plan", {}, known), { source: "/v/promo.mp4" })
  assert.deepEqual(fillSlots("video.film.plan", {}, { project: "f-1" }), {})
  // the caller's own values win
  assert.deepEqual(fillSlots("video.stage.qa", { project: "other", source: "/x.mp4" }, known), { project: "other", source: "/x.mp4" })
  assert.deepEqual(fillSlots("video.stage.qa"), {})
})
