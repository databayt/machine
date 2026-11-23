// production — what the strip reads, as plain data. Pure: no React, no IPC (node --test src/components/production/pure.test.ts).
// Every parser is tolerant: a field kun adds or drops never throws, it just leaves the part out.

/** The line's 14 stages, in order. */
export const STAGES = ["request", "research", "brief", "script", "storyboard", "assets", "visual", "audio", "edit", "render", "qa", "approval", "cdn", "publish"] as const
export type StageName = (typeof STAGES)[number]

export interface Stage {
  /** 1-based, the number the line prints (`07`). */
  n: number
  name: string
  done: boolean
  file: string | null
  at: string | null
  /** Variants that have this stage done, when the status carries a variant grid. */
  variants: { done: number; total: number } | null
  /** The stage or one of its variants failed (a QA fail): the red light. */
  failed: boolean
}

/** A stage's light: green done, orange next, red failed, grey not started. */
export type Light = "green" | "orange" | "red" | "grey"
export function lightOf(stage: Stage, isNext: boolean): Light {
  if (stage.failed) return "red"
  if (stage.done) return "green"
  return isNext ? "orange" : "grey"
}

export interface Gate {
  name: string
  signed: boolean
  by: string | null
  at: string | null
}

export interface FilmSummary {
  id: string
  title: string
  brand: string
  slug: string
  version: string
  status: string
  /** `updatedAt`, else `createdAt` (ISO), for the default pick. */
  updated: string
  next: string | null
  focus: string[]
}

export interface Film {
  id: string
  title: string
  brand: string
  /** The brand's on-screen identity (`balqalam`), falling back to the brand. */
  identity: string
  version: string
  status: string
  slug: string
  stages: Stage[]
  next: Stage | null
  gates: Gate[]
  variants: string[]
  /** The language of the focus variant (`sd-ar` -> `ar`), when there is one. */
  lang: string | null
}

type Obj = Record<string, unknown>
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v)
const str = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v : null)
const strs = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [])

const DONE_WORDS = new Set(["done", "approved", "complete", "completed", "ok", "pass", "passed", "signed", "published", "yes"])
/** A stage or gate cell: true, a done-ish word, or an object with a done-ish `status`. */
export function isDone(v: unknown): boolean {
  if (v === true) return true
  if (typeof v === "string") return DONE_WORDS.has(v.toLowerCase())
  if (isObj(v)) {
    if (v.done === true) return true
    const s = str(v.status)
    return s ? DONE_WORDS.has(s.toLowerCase()) : false
  }
  return false
}

/** `hogwarts-intro-introduction-v001` -> `intro-introduction`: the id without its brand and version. */
export function slugOf(id: string, brand = ""): string {
  let s = id.replace(/-v\d+$/i, "")
  if (brand && s.startsWith(`${brand}-`)) s = s.slice(brand.length + 1)
  else s = s.replace(/^[a-z0-9]+-/, "")
  return s || id
}

/** The films of `produce list --json`; anything that is not a film is skipped. Newest first. */
export function parseList(raw: unknown): FilmSummary[] {
  if (!Array.isArray(raw)) return []
  const films: FilmSummary[] = []
  for (const r of raw) {
    if (!isObj(r)) continue
    const id = str(r.id)
    if (!id) continue
    const brand = str(r.brand) ?? ""
    const category = str(r.category)
    const subject = str(r.subject)
    films.push({
      id,
      title: str(r.title) ?? id,
      brand,
      slug: category && subject ? `${category}-${subject}` : slugOf(id, brand),
      version: str(r.version) ?? "",
      status: str(r.status) ?? "",
      updated: str(r.updatedAt) ?? str(r.updated) ?? str(r.createdAt) ?? "",
      next: str(r.next),
      focus: strs(r.focus),
    })
  }
  return films.sort((a, b) => b.updated.localeCompare(a.updated))
}

/** The film the strip opens on: the most recently updated (the list is already newest first). */
export function defaultFilm(films: readonly FilmSummary[], keep?: string | null): string | null {
  if (keep && films.some((f) => f.id === keep)) return keep
  return films[0]?.id ?? null
}

/** The stages of a status, in line order: a map (`{ request: true }` or `{ request: { status, file, at } }`) or a list of `{ name, … }`. */
function stageCells(raw: unknown): Map<string, unknown> {
  const out = new Map<string, unknown>()
  if (Array.isArray(raw)) {
    for (const s of raw) {
      if (!isObj(s)) continue
      const name = str(s.name) ?? str(s.stage) ?? str(s.id)
      if (name) out.set(name, s)
    }
  } else if (isObj(raw)) for (const [k, v] of Object.entries(raw)) out.set(k, v)
  return out
}

/** Per-stage variant counts from a grid `{ "sd-ar": { audio: "done" } }` or a list `[{ id, stages: {…} }]`. */
function variantGrid(raw: unknown): { ids: string[]; cells: Map<string, Map<string, unknown>> } {
  const ids: string[] = []
  const cells = new Map<string, Map<string, unknown>>()
  const put = (id: string, stages: unknown) => {
    ids.push(id)
    cells.set(id, stageCells(stages))
  }
  if (Array.isArray(raw)) {
    for (const v of raw) {
      if (typeof v === "string") ids.push(v)
      else if (isObj(v)) {
        const id = str(v.id) ?? str(v.variant)
        if (id) put(id, v.stages ?? v.status)
      }
    }
  } else if (isObj(raw)) for (const [id, stages] of Object.entries(raw)) put(id, stages)
  return { ids, cells }
}

/** The gates: a map (`{ look: null, script: { by, at } }`) or a list of `{ name, … }`. */
export function parseGates(raw: unknown): Gate[] {
  const cells = stageCells(raw)
  const gates: Gate[] = []
  for (const [name, v] of cells) {
    const o = isObj(v) ? v : null
    gates.push({ name, signed: isDone(v) || !!str(o?.by), by: str(o?.by) ?? (typeof v === "string" && !DONE_WORDS.has(v.toLowerCase()) ? v : null), at: str(o?.at) })
  }
  return gates
}

/** One film's status: tolerant of the shape. Null when it carries no id. */
export function parseStatus(raw: unknown): Film | null {
  if (!isObj(raw)) return null
  const id = str(raw.id)
  if (!id) return null
  const brand = str(raw.brand) ?? ""
  const cells = stageCells(raw.stages)
  const grid = variantGrid(raw.variantGrid ?? raw.grid ?? raw.variants)
  const stages: Stage[] = []
  const known = [...STAGES, ...[...cells.keys()].filter((k) => !(STAGES as readonly string[]).includes(k))]
  for (const name of known) {
    const cell = cells.get(name)
    const o = isObj(cell) ? cell : null
    let variants: Stage["variants"] = null
    if (grid.cells.size > 0) {
      const have = [...grid.cells.values()].filter((m) => m.has(name))
      if (have.length > 0) variants = { done: have.filter((m) => isDone(m.get(name))).length, total: grid.ids.length }
    }
    const failedVariant = [...grid.cells.values()].some((m) => m.get(name) === "failed")
    const failed = str(o?.status) === "failed" || cell === "failed" || failedVariant
    stages.push({ n: stages.length + 1, name, done: isDone(cell), file: str(o?.file), at: str(o?.at), variants, failed })
  }
  const nextName = str(raw.next)
  const next = (nextName ? stages.find((s) => s.name === nextName) : null) ?? stages.find((s) => !s.done) ?? null
  const focus = strs(raw.focus)[0] ?? str(raw.focus) ?? ""
  return {
    id,
    title: str(raw.title) ?? id,
    brand,
    identity: str(raw.identity) ?? brand,
    version: str(raw.version) ?? "",
    status: str(raw.status) ?? "",
    slug: str(raw.slug) ?? slugOf(id, brand),
    stages,
    next,
    gates: parseGates(raw.gates),
    variants: grid.ids,
    lang: /-(ar|en)$/.exec(focus)?.[1] ?? null,
  }
}

export interface Collapsed {
  /** One dot per stage: filled when done. */
  dots: boolean[]
  /** `07 visual`, or null when every stage is done. */
  next: string | null
  gates: { name: string; signed: boolean }[]
}

/** The collapsed strip's parts: `●●●●●●○○○○○○○○ 07 visual · script ✔ look ○`. */
export function collapsed(film: Film): Collapsed {
  return {
    dots: film.stages.map((s) => s.done),
    next: film.next ? `${String(film.next.n).padStart(2, "0")} ${film.next.name}` : null,
    gates: film.gates.map((g) => ({ name: g.name, signed: g.signed })),
  }
}

/** The text of the collapsed line, for the title attribute and the tests. */
export function collapsedText(film: Film): string {
  const c = collapsed(film)
  const dots = c.dots.map((d) => (d ? "●" : "○")).join("")
  const gates = c.gates.map((g) => `${g.name} ${g.signed ? "✔" : "○"}`).join(" ")
  return [`▸ ${film.slug}`, `${dots}${c.next ? ` ${c.next}` : ""}`, gates].filter(Boolean).join(" · ")
}

/** The stage a human gate signs off, by the line's convention: script review, the look review, the version approval, publish. */
export const GATE_STAGE: Record<string, string> = { script: "script", look: "visual", version: "approval", publish: "publish" }

/** The gate that sits on a stage row, if any. */
export function gateOf(film: Film, stage: string): Gate | null {
  return film.gates.find((g) => GATE_STAGE[g.name] === stage) ?? null
}

// ---- scenes -------------------------------------------------------------------------------------

export type SceneKind = "capture" | "3d" | "image" | "video" | "graphic" | "other"

/** `capture`, `graphic`, `3d`… from a shot's `source.type`, whatever the spelling. */
export function sceneKind(type: unknown): SceneKind {
  const t = (str(type) ?? "").toLowerCase()
  if (/capture|screen|record|flow|shoot/.test(t)) return "capture"
  if (/3d|blender|glb|render/.test(t)) return "3d"
  if (/image|still|photo|nano|picture/.test(t)) return "image"
  if (/video|footage|clip|veo|omni/.test(t)) return "video"
  if (/graphic|hyperframes|motion|type|text/.test(t)) return "graphic"
  return "other"
}

const FILE_KINDS: [RegExp, SceneKind][] = [
  [/\.(png|jpe?g|webp|gif|svg|avif)$/i, "image"],
  [/\.(mp4|mov|webm|m4v)$/i, "video"],
  [/\.(glb|gltf|blend|usdz|obj)$/i, "3d"],
]
/** What a file is, by its extension: an image, a video or a 3D model; `other` for the rest. */
export function fileKind(path: string): SceneKind {
  return FILE_KINDS.find(([re]) => re.test(path))?.[1] ?? "other"
}

export interface SceneFile {
  path: string
  kind: SceneKind
  lang: string | null
}

export interface Scene {
  id: string
  kind: SceneKind
  seconds: number | null
  /** The narration line, in the language asked (else the other). */
  line: string
  visual: string
  flow: string | null
  files: SceneFile[]
}

/** The files a manifest lists for one scene: `{ ar: [paths], en: [paths] }`, a bare list, or `{ files: […] }`. */
function manifestFiles(v: unknown): SceneFile[] {
  const out: SceneFile[] = []
  const add = (p: unknown, lang: string | null) => {
    if (typeof p === "string" && p.trim()) out.push({ path: p, kind: fileKind(p), lang })
    else if (isObj(p)) add(p.path ?? p.file ?? p.src, lang)
  }
  if (Array.isArray(v)) v.forEach((p) => add(p, null))
  else if (isObj(v)) {
    for (const [k, x] of Object.entries(v)) {
      const lang = /^[a-z]{2}$/.test(k) ? k : null
      if (Array.isArray(x)) x.forEach((p) => add(p, lang))
      else if (k === "files" || k === "assets") manifestFiles(x).forEach((f) => out.push(f))
      else add(x, lang)
    }
  }
  return out
}

/** The manifest's per-scene file lists, by scene id: `scenes: { id: … }`, or `scenes: [{ id|scene, … }]`. */
export function manifestByScene(manifest: unknown): Map<string, SceneFile[]> {
  const out = new Map<string, SceneFile[]>()
  if (!isObj(manifest)) return out
  const scenes = manifest.scenes ?? manifest.assets
  if (Array.isArray(scenes)) {
    for (const s of scenes) {
      if (!isObj(s)) continue
      const id = str(s.id) ?? str(s.scene)
      if (id) out.set(id, [...(out.get(id) ?? []), ...manifestFiles(s.files ?? s.assets ?? s.paths ?? s)])
    }
  } else if (isObj(scenes)) for (const [id, v] of Object.entries(scenes)) out.set(id, manifestFiles(v))
  return out
}

/** The shot list × the asset manifest: one row per scene, with its files. Empty when there is no shot list. */
export function parseScenes(shotList: unknown, manifest: unknown, lang: "ar" | "en" = "en"): Scene[] {
  if (!isObj(shotList) || !Array.isArray(shotList.scenes)) return []
  const files = manifestByScene(manifest)
  const scenes: Scene[] = []
  for (const s of shotList.scenes) {
    if (!isObj(s)) continue
    const id = str(s.id)
    if (!id) continue
    const nar = s.narration
    const line = isObj(nar) ? (str(nar[lang]) ?? str(nar[lang === "ar" ? "en" : "ar"]) ?? "") : (str(nar) ?? "")
    const src = isObj(s.source) ? s.source : null
    scenes.push({
      id,
      kind: sceneKind(src?.type ?? s.type),
      seconds: typeof s.durationS === "number" ? s.durationS : typeof s.duration === "number" ? s.duration : null,
      line,
      visual: str(s.visual) ?? "",
      flow: str(s.flow),
      files: files.get(id) ?? [],
    })
  }
  return scenes
}

/** The scene's files for one language: that language's, plus the untagged; every file when none is tagged for it. */
export function filesFor(scene: Scene, lang: string | null): SceneFile[] {
  if (!lang) return scene.files
  const mine = scene.files.filter((f) => f.lang === null || f.lang === lang)
  return mine.length > 0 ? mine : scene.files
}

// ---- formulas -----------------------------------------------------------------------------------

/** The formula a stage row fills the composer with. */
export const stageFormula = (name: string): string => `video.stage.${name}`

/** The work-order formula for a scene's kind; a graphic, or a kind with no order, falls to the visual stage. */
export function orderFormula(kind: SceneKind): string {
  switch (kind) {
    case "capture":
      return "video.order.capture"
    case "3d":
      return "video.order.blender"
    case "image":
      return "video.order.image"
    case "video":
      return "video.order.filmcraft"
    default:
      return stageFormula("visual")
  }
}

/** The slots a click hands to `fill`: the film, and for a scene its id and flow. */
export function slotsFor(film: string, scene?: Pick<Scene, "id" | "flow"> | null): Record<string, string> {
  const set: Record<string, string> = { project: film }
  if (scene) {
    set.scene = scene.id
    if (scene.flow) set.flow = scene.flow
  }
  return set
}

/** Formulas that work on an existing film: they take the selected film as `project`. `video.film.plan` starts a new one and does not. */
const FILM_FORMULA = /^video\.(stage|order|gate)\.|^video\.film\.continue$/

/**
 * The slots a fill gets: the caller's own, plus the selected film as `project` for a formula on an existing film, plus
 * the attached video as `source` (a formula without that slot ignores it). The caller's own values win.
 */
export function fillSlots(
  id: string,
  set: Record<string, string> = {},
  known: { project?: string | null; source?: string | null; video?: Record<string, string> | null } = {},
): Record<string, string> {
  // The film's Format/Style settings go to every video formula; an explicit slot still wins.
  const out = known.video && id.startsWith("video.") ? { ...known.video, ...set } : { ...set }
  if (known.project && !out.project && FILM_FORMULA.test(id)) out.project = known.project
  if (known.source && !out.source) out.source = known.source
  return out
}

/** `source` joins the slots when the formula declares it and a video was attached. */
export function withSource(set: Record<string, string>, source: string | null | undefined): Record<string, string> {
  return source ? { ...set, source } : set
}

// ---- brand --------------------------------------------------------------------------------------

export interface Brand {
  identity: string
  logo: string | null
  fonts: { role: string; name: string }[]
  palette: { name: string; hex: string }[]
  voice: string | null
}

/** `thmanyah-serif-display-900.ttf` -> `Thmanyah Serif Display 900`; a plain name stays. */
export function fontName(file: string): string {
  const base = file.replace(/\.(ttf|otf|woff2?)$/i, "")
  if (base === file && !/[-_]/.test(file)) return file
  return base.split(/[-_]/).filter(Boolean).map((w) => (/^\d+$/.test(w) ? w : w[0]!.toUpperCase() + w.slice(1))).join(" ")
}

/** The brand kit `production_brand` returns. Null when it has no identity. */
export function parseBrand(raw: unknown): Brand | null {
  if (!isObj(raw)) return null
  const identity = str(raw.identity)
  if (!identity) return null
  const fonts = isObj(raw.fonts) ? Object.entries(raw.fonts).flatMap(([role, v]) => (str(v) ? [{ role, name: fontName(v as string) }] : [])) : []
  const palette = isObj(raw.palette) ? Object.entries(raw.palette).flatMap(([name, v]) => (str(v)?.startsWith("#") ? [{ name, hex: v as string }] : [])) : []
  return { identity, logo: str(raw.logoData), fonts, palette, voice: str(raw.voice) }
}

// ---- stage groups -------------------------------------------------------------------------------

/** The 14 stages in four groups, the panel's tabs: idea → story → make → ship. */
export const STAGE_GROUPS = [
  { id: "idea", stages: ["request", "research", "brief"] },
  { id: "story", stages: ["script", "storyboard"] },
  { id: "make", stages: ["assets", "visual", "audio", "edit"] },
  { id: "ship", stages: ["render", "qa", "approval", "cdn", "publish"] },
] as const

export type GroupId = (typeof STAGE_GROUPS)[number]["id"]

/** The group a stage belongs to; an unknown stage falls in "make". */
export function groupOf(stage: string): GroupId {
  return STAGE_GROUPS.find((g) => (g.stages as readonly string[]).includes(stage))?.id ?? "make"
}

/** A group's stages from the film, in order; stages the film doesn't have are left out. */
export function groupStages(film: Film, group: GroupId): Stage[] {
  const names = STAGE_GROUPS.find((g) => g.id === group)?.stages ?? []
  return names.map((n) => film.stages.find((s) => s.name === n)).filter((s): s is Stage => !!s)
}

// ---- video settings (the Format and Style tabs) -------------------------------------------------

export interface VideoSettings {
  ratio: string
  length: string
  resolution: string
  fps: string
  platform: string
  look: string
  voice: string
  captions: string
  music: string
  version: string
}

/** Each setting's choices, in the order the chips show. The first is the default unless the film says otherwise. */
export const SETTING_CHOICES = {
  ratio: ["9:16", "4:5", "1:1", "16:9"],
  length: ["source", "0:15", "0:30", "1:00", "2:00"],
  resolution: ["1080p", "4K"],
  fps: ["30", "60"],
  platform: ["Facebook", "Instagram", "TikTok", "YouTube"],
  look: ["mixed", "captures", "3D"],
  voice: ["source", "stock", "none"],
  captions: ["burned in", "off"],
  music: ["brand bed", "none"],
  version: ["sd-ar", "sa-ar", "ae-ar", "eg-ar", "sd-en", "sa-en", "rw-en"],
} as const satisfies Record<keyof VideoSettings, readonly string[]>

export const FORMAT_KEYS = ["ratio", "length", "resolution", "fps", "platform"] as const
export const STYLE_KEYS = ["look", "voice", "captions", "music", "version"] as const

/** The film's defaults: its first variant picks the version; everything else starts at the first choice. */
export function defaultSettings(variants: readonly string[] = []): VideoSettings {
  const v = variants.find((x) => (SETTING_CHOICES.version as readonly string[]).includes(x)) ?? "sd-ar"
  return { ratio: "9:16", length: "source", resolution: "1080p", fps: "30", platform: "Facebook", look: "mixed", voice: "source", captions: "burned in", music: "brand bed", version: v }
}

/** Saved settings over the defaults; unknown keys and values outside the choices are dropped. */
export function mergeSettings(base: VideoSettings, saved: unknown): VideoSettings {
  if (!isObj(saved)) return base
  const out = { ...base }
  for (const k of Object.keys(SETTING_CHOICES) as (keyof VideoSettings)[]) {
    const v = saved[k]
    if (typeof v === "string" && (SETTING_CHOICES[k] as readonly string[]).includes(v)) out[k] = v
  }
  return out
}

const RATIO_FORMAT: Record<string, string> = {
  "9:16": "vertical 9:16, mobile",
  "4:5": "portrait 4:5, mobile",
  "1:1": "square 1:1, mobile",
  "16:9": "horizontal 16:9, desktop",
}

/** The settings as formula slots: format, length, platform, market, and one `settings` line for the rest. */
export function settingsSlots(s: VideoSettings): Record<string, string> {
  const [market] = s.version.split("-")
  return {
    format: RATIO_FORMAT[s.ratio] ?? s.ratio,
    length: s.length === "source" ? "match the source video" : `about ${s.length}`,
    platform: s.platform,
    market,
    settings: [
      s.resolution,
      `${s.fps} fps`,
      `version ${s.version}`,
      `look ${s.look}`,
      `voice ${s.voice === "source" ? "source voiceover" : s.voice === "stock" ? "stock voice" : "none"}`,
      `captions ${s.captions}`,
      `music ${s.music}`,
    ].join(" · "),
  }
}
