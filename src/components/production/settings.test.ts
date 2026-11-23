// node --test src/components/production/settings.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import { SETTING_CHOICES, defaultSettings, settingsSlots } from "./pure.ts"
import { FALLBACK_SETTINGS, composeLocal, initialPicks, optionFits, parseCompose, parseSettings, pickKey, visibleOptions, withoutNew } from "./settings-pure.ts"

const L = (en: string) => ({ en, ar: en })
const RAW = {
  dept: "video",
  tabs: [
    { id: "film", label: L("Film"), source: "ledger" },
    { id: "kind", label: L("Kind"), groups: [{ key: "kind", label: L("Kind"), q: L("What kind?"), stage: "request", default: "intro", current: "intro", options: [
      { value: "intro", label: L("Intro film"), say: L("Intro film"), hint: null, when: "new" },
      { value: "versions", label: L("All versions"), say: L("All versions"), hint: L("one per market"), when: "film" },
      { value: "any", label: L("Any"), when: null },
    ] }] },
    { id: "style", label: L("Style"), groups: [{ key: "look", label: { en: "Look" }, stage: "visual", default: "mixed", options: [{ value: "mixed", label: L("Mixed") }, { value: "3D", label: "3D" }] }] },
    { id: "empty", label: L("Empty") },
    { id: "stages", label: L("Stages"), source: "ledger" },
  ],
  sentence: L("{kind} · {look}"),
}

test("parseSettings reads tabs with groups or a source and drops the rest", () => {
  const d = parseSettings(RAW)!
  assert.deepEqual(d.tabs.map((t) => t.id), ["film", "kind", "style", "stages"])
  assert.equal(d.tabs[0]!.source, "ledger")
  const kind = d.tabs[1]!.groups[0]!
  assert.deepEqual([kind.key, kind.stage, kind.default, kind.current], ["kind", "request", "intro", "intro"])
  assert.equal(kind.options[0]!.when, "new")
  assert.equal(kind.options[2]!.when, null)
  assert.equal(kind.options[1]!.hint?.en, "one per market")
  // tolerant: a one-language label fills the other, a plain-string label works, current falls to the default
  const look = d.tabs[2]!.groups[0]!
  assert.deepEqual(look.label, { en: "Look", ar: "Look" })
  assert.equal(look.current, "mixed")
  assert.equal(look.options[1]!.label.en, "3D")
  assert.equal(d.sentence?.en, "{kind} · {look}")
})

test("parseSettings is null for an error, no tabs or junk", () => {
  for (const raw of [null, "x", [], {}, { error: "no settings" }, { tabs: [] }, { tabs: [{ id: "x" }] }]) assert.equal(parseSettings(raw), null, JSON.stringify(raw))
})

test("a Kind option is offered by its `when`: new with no film, film with one, no `when` always", () => {
  const kind = parseSettings(RAW)!.tabs[1]!.groups[0]!
  assert.deepEqual(visibleOptions(kind, false).map((o) => o.value), ["intro", "any"])
  assert.deepEqual(visibleOptions(kind, true).map((o) => o.value), ["versions", "any"])
  assert.ok(optionFits({ when: null }, true) && !optionFits({ when: "new" }, true))
})

test("initialPicks: defaults for a new film; an open film has no kind unless one was picked", () => {
  const d = parseSettings(RAW)!
  assert.deepEqual(initialPicks(d, false), { kind: "intro", look: "mixed" })
  assert.deepEqual(initialPicks(d, true), { look: "mixed" })
  assert.deepEqual(initialPicks(d, true, { kind: "versions", look: "gone" }), { kind: "versions", look: "mixed" })
  assert.deepEqual(initialPicks(d, false, { look: "3D", kind: "versions" }), { kind: "intro", look: "3D" })
})

test("parseCompose reads the request and the reserved `new` slot goes away with withoutNew", () => {
  const c = parseCompose({ formula: "video.film.plan", route: "plan", record: "film", picks: { kind: "intro" }, set: { film_type: "intro", new: "1", n: 3 }, sentence: L("Intro film · owners"), keywords: ["produce"], tools: ["shoot"], workflow: "film-line", missing: [] })!
  assert.equal(c.formula, "video.film.plan")
  assert.deepEqual(c.set, { film_type: "intro", new: "1" })
  assert.equal(c.sentence.en, "Intro film · owners")
  assert.deepEqual(withoutNew(c.set), { film_type: "intro" })
  assert.equal(parseCompose({ formula: null, set: {} })!.formula, null)
  assert.equal(parseCompose({ error: "x" }), null)
  assert.equal(parseCompose(7), null)
})

test("the fallback carries today's choices on the Film, Format, Style and Stages tabs", () => {
  assert.deepEqual(FALLBACK_SETTINGS.tabs.map((t) => t.id), ["film", "format", "style", "stages"])
  const groups = FALLBACK_SETTINGS.tabs.flatMap((t) => t.groups)
  assert.deepEqual(groups.map((g) => g.key), ["ratio", "length", "resolution", "fps", "platform", "look", "voice", "captions", "music", "version"])
  for (const g of groups) {
    assert.deepEqual(g.options.map((o) => o.value), [...SETTING_CHOICES[g.key as keyof typeof SETTING_CHOICES]])
    assert.equal(g.default, defaultSettings()[g.key as keyof typeof SETTING_CHOICES])
  }
  assert.equal(groups.find((g) => g.key === "fps")!.options[0]!.label.en, "30 fps")
  assert.equal(groups.find((g) => g.key === "length")!.options[0]!.label.en, "match source")
})

test("composeLocal: a new film plans with today's settings slots, an open film composes nothing", () => {
  const picks = { ratio: "16:9", look: "3D", voice: "none" }
  const c = composeLocal(FALLBACK_SETTINGS, picks)
  assert.equal(c.formula, "video.film.plan")
  assert.equal(c.record, "film")
  assert.deepEqual(c.set, settingsSlots({ ...defaultSettings(), ratio: "16:9", look: "3D", voice: "none" }))
  assert.ok(!("new" in c.set))
  assert.ok(c.sentence.en.includes("16:9") && c.sentence.en.includes("3D"))
  const open = composeLocal(FALLBACK_SETTINGS, picks, "film-1")
  assert.equal(open.formula, null)
  assert.deepEqual(open.set, c.set, "the slots still feed every later fill")
})

test("a new film's choices are kept under their own key", () => {
  assert.equal(pickKey(null), "kun.video.settings._new")
  assert.equal(pickKey("f-1"), "kun.video.settings.f-1")
})
