// production — the settings panel as data: the tabs and choices `station settings` returns, the request
// `station compose` writes from them, and the choices hard-coded today as the fallback for an older kun.
// Pure: no React, no IPC (node --test src/components/production/settings.test.ts).
import { copy } from "./config.ts"
import { SETTING_CHOICES, type VideoSettings, defaultSettings, mergeSettings, settingsSlots } from "./pure.ts"

export interface L {
  en: string
  ar: string
}

export interface SettingOption {
  value: string
  label: L
  say: L | null
  hint: L | null
  /** `new`: offered with no film selected; `film`: with one; null: both. */
  when: "new" | "film" | null
}

export interface SettingGroup {
  key: string
  label: L
  q: L | null
  /** The stage that owns the choice (changing it on an open film re-locks that stage). */
  stage: string | null
  default: string | null
  /** The film's locked value (or the default). */
  current: string | null
  options: SettingOption[]
}

export interface SettingTab {
  id: string
  label: L
  /** `ledger`: the tab is drawn from the film ledger (the films, the stages), not from choices. */
  source: string | null
  groups: SettingGroup[]
}

export interface SettingsDoc {
  tabs: SettingTab[]
  sentence: L | null
}

type Obj = Record<string, unknown>
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v)
const str = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v : null)

/** `{ en, ar }` (either may be missing: the other fills in) or a plain string. */
function lab(v: unknown): L | null {
  if (typeof v === "string") return v.trim() ? { en: v, ar: v } : null
  if (!isObj(v)) return null
  const en = str(v.en)
  const ar = str(v.ar)
  const one = en ?? ar
  return one ? { en: en ?? one, ar: ar ?? one } : null
}

function parseOption(raw: unknown): SettingOption | null {
  if (!isObj(raw)) return null
  const value = str(raw.value)
  if (!value) return null
  return {
    value,
    label: lab(raw.label) ?? { en: value, ar: value },
    say: lab(raw.say),
    hint: lab(raw.hint),
    when: raw.when === "new" || raw.when === "film" ? raw.when : null,
  }
}

function parseGroup(raw: unknown): SettingGroup | null {
  if (!isObj(raw)) return null
  const key = str(raw.key)
  if (!key || !Array.isArray(raw.options)) return null
  const options = raw.options.map(parseOption).filter((o): o is SettingOption => o !== null)
  if (options.length === 0) return null
  return { key, label: lab(raw.label) ?? { en: key, ar: key }, q: lab(raw.q), stage: str(raw.stage), default: str(raw.default), current: str(raw.current) ?? str(raw.default), options }
}

/** The `station settings` JSON. Tolerant: a tab with neither groups nor a source is dropped; null when no tab is left. */
export function parseSettings(raw: unknown): SettingsDoc | null {
  if (!isObj(raw) || raw.error || !Array.isArray(raw.tabs)) return null
  const tabs: SettingTab[] = []
  for (const t of raw.tabs) {
    if (!isObj(t)) continue
    const id = str(t.id)
    if (!id) continue
    const groups = Array.isArray(t.groups) ? t.groups.map(parseGroup).filter((g): g is SettingGroup => g !== null) : []
    const source = str(t.source)
    if (!source && groups.length === 0) continue
    tabs.push({ id, label: lab(t.label) ?? { en: id, ar: id }, source, groups })
  }
  return tabs.length ? { tabs, sentence: lab(raw.sentence) } : null
}

/** What `station compose` made of the choices. `formula` is null when they compose no request. */
export interface Composed {
  formula: string | null
  route: string | null
  record: string | null
  picks: Record<string, string>
  /** Formula slots; may hold the reserved `new: "1"`, which only a fill may carry. */
  set: Record<string, string>
  sentence: L
  keywords: string[]
  tools: string[]
  workflow: string | null
  missing: string[]
}

function strMap(v: unknown): Record<string, string> {
  const out: Record<string, string> = {}
  if (isObj(v)) for (const [k, x] of Object.entries(v)) if (typeof x === "string") out[k] = x
  return out
}

const strs = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [])

/** The `station compose` JSON; null on an error answer or anything that is not an object. */
export function parseCompose(raw: unknown): Composed | null {
  if (!isObj(raw) || raw.error) return null
  return {
    formula: str(raw.formula),
    route: str(raw.route),
    record: str(raw.record),
    picks: strMap(raw.picks),
    set: strMap(raw.set),
    sentence: lab(raw.sentence) ?? { en: "", ar: "" },
    keywords: strs(raw.keywords),
    tools: strs(raw.tools),
    workflow: str(raw.workflow),
    missing: strs(raw.missing),
  }
}

/** `new` is reserved: it goes to a fill only, never into the slots every later fill inherits. */
export function withoutNew(set: Record<string, string>): Record<string, string> {
  const { new: _n, ...rest } = set
  return rest
}

/** Where the choices of a NEW film are kept on this computer (an open film has its own). */
export const pickKey = (film: string | null): string => `kun.video.settings.${film ?? "_new"}`

/** Whether an option is offered: `new` with no film selected, `film` with one, no `when` always. */
export const optionFits = (o: Pick<SettingOption, "when">, hasFilm: boolean): boolean => o.when === null || (hasFilm ? o.when === "film" : o.when === "new")

/** The options of a group the person may pick now. */
export const visibleOptions = (g: SettingGroup, hasFilm: boolean): SettingOption[] => g.options.filter((o) => optionFits(o, hasFilm))

const groupsOf = (doc: SettingsDoc): SettingGroup[] => doc.tabs.flatMap((t) => t.groups)

/**
 * The choices on screen: the film's current values (or the defaults), over which saved picks lie when they are
 * still offered. A kind that does not fit (an open film has none by default) is left out.
 */
export function initialPicks(doc: SettingsDoc, hasFilm: boolean, saved?: unknown): Record<string, string> {
  const out: Record<string, string> = {}
  const mine = isObj(saved) ? saved : {}
  for (const g of groupsOf(doc)) {
    const fits = visibleOptions(g, hasFilm)
    const ok = (v: unknown): v is string => typeof v === "string" && fits.some((o) => o.value === v)
    const v = ok(mine[g.key]) ? mine[g.key] : ok(g.current) && !(hasFilm && g.key === "kind") ? g.current : ok(g.default) && !hasFilm ? g.default : null
    if (typeof v === "string") out[g.key] = v
  }
  return out
}

// ---- the fallback: the choices hard-coded today (an older kun has no `station settings`) ----------

const FALLBACK_TABS: { id: "format" | "style"; keys: (keyof VideoSettings)[] }[] = [
  { id: "format", keys: ["ratio", "length", "resolution", "fps", "platform"] },
  { id: "style", keys: ["look", "voice", "captions", "music", "version"] },
]

/** A chip's text: lengths and voices read as words, the rest as they are. */
function fallbackLabel(k: keyof VideoSettings, v: string): L {
  const w = (c: L): L => c
  if (k === "length" && v === "source") return w(copy.matchSource)
  if (k === "fps") return { en: `${v} fps`, ar: `${v} fps` }
  if (k === "voice") return w(v === "source" ? copy.voiceSource : v === "stock" ? copy.voiceStock : copy.none)
  if ((k === "music" || k === "captions") && (v === "none" || v === "off")) return w(copy.off)
  return { en: v, ar: v }
}

const STAGE_OF: Record<keyof VideoSettings, string> = { ratio: "request", length: "request", platform: "request", version: "request", resolution: "render", fps: "render", look: "visual", voice: "audio", music: "audio", captions: "edit" }

export const FALLBACK_SETTINGS: SettingsDoc = {
  tabs: [
    { id: "film", label: copy.film, source: "ledger", groups: [] },
    ...FALLBACK_TABS.map((t) => ({
      id: t.id,
      label: copy[t.id],
      source: null,
      groups: t.keys.map((k): SettingGroup => {
        const d = defaultSettings()[k]
        return {
          key: k,
          label: copy[k],
          q: null,
          stage: STAGE_OF[k],
          default: d,
          current: d,
          options: SETTING_CHOICES[k].map((v) => ({ value: v, label: fallbackLabel(k, v), say: null, hint: null, when: null })),
        }
      }),
    })),
    { id: "stages", label: copy.stages, source: "ledger", groups: [] },
  ],
  sentence: null,
}

/**
 * The request the fallback composes: only for a NEW film (`video.film.plan`); an open film composes none. The slots
 * are today's `settingsSlots`, and the sentence is the picked labels.
 */
export function composeLocal(doc: SettingsDoc, picks: Record<string, string>, film: string | null = null): Composed {
  const s = mergeSettings(defaultSettings(), picks)
  const set = settingsSlots(s)
  const label = (k: keyof VideoSettings): string => {
    const g = groupsOf(doc).find((x) => x.key === k)
    return g?.options.find((o) => o.value === s[k])?.label.en ?? s[k]
  }
  const sentence = [`${label("platform")} ${label("ratio")}`, label("length"), label("look"), label("voice")].join(" · ")
  return {
    formula: film ? null : "video.film.plan",
    route: null,
    record: film ? null : "film",
    picks: { ...s },
    set,
    sentence: { en: sentence, ar: sentence },
    keywords: [],
    tools: [],
    workflow: null,
    missing: [],
  }
}
