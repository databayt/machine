// shell — the one navigation tree (the sidebar, the menu footer and the Settings cards read it) and
// the pure function that turns the address into "where am I". No React here, so node can test it.
import type { Copy } from "../chat/config.ts"
import { APP_STAGES, APPS } from "../apps/registry.ts"
import { copy } from "../menu/config.ts"
import { CONNECTORS } from "../plugins/registry.ts"
import type { Stage, StepStatus } from "../readiness/types.ts"

export type ParentId = "sessions" | "plugins" | "apps" | "library" | "machines" | "setup" | "settings"

export interface NavNode {
  id: string
  /** A translated pair, or a literal (folder names, stage titles already resolved). */
  label: Copy | string
  href: (lang: string) => string
  /** A muted count after the label (`done/total`). */
  hint?: string
  children?: NavNode[]
}

export interface Trail {
  parent: ParentId | null
  /** Level 1 id: a stage, a folder, a Settings view. */
  child: string | null
  /** Level 2 id: a step, a catalog item. */
  leaf: string | null
}

/** The `.claude` folders the Library lists, in order (`mcp` is Plugins now). */
export const CLAUDE_DIRS = ["skills", "agents", "rules", "hooks", "departments"] as const
export type ClaudeDir = (typeof CLAUDE_DIRS)[number]

/** The registry type each folder shows. */
const TYPE_OF_DIR: Record<ClaudeDir, string> = {
  skills: "skill",
  agents: "agent",
  rules: "rule",
  hooks: "hook",
  departments: "pack",
}

export const typeOfDir = (dir: string): string | null => TYPE_OF_DIR[dir as ClaudeDir] ?? null

/** The folder an item id (`skill:funnel`) lives in. */
export function dirOfItem(id: string): ClaudeDir | null {
  const type = id.split(":")[0]
  return CLAUDE_DIRS.find((d) => TYPE_OF_DIR[d] === type) ?? null
}

/** The part of an item id after the colon. */
export const itemName = (id: string): string => (id.includes(":") ? id.slice(id.indexOf(":") + 1) : id)

/** `/ar/setup/` → { lang: "ar", rest: "/setup" } */
export function splitPath(pathname: string): { lang: string; rest: string } {
  const [, lang = "", ...more] = pathname.replace(/\/+$/, "").split("/")
  return { lang, rest: more.length ? `/${more.join("/")}` : "" }
}

type Params = { get(name: string): string | null }

const SETTINGS_ROUTES: Record<string, string> = { "/model": "model", "/config": "config", "/health": "health", "/engine": "engine" }

/** Where the address is in the tree. Anything outside the seven parents (the chat, a typo) is all null. */
export function trailOf(pathname: string, params: Params): Trail {
  const { rest } = splitPath(pathname)
  const none: Trail = { parent: null, child: null, leaf: null }
  if (rest === "/sessions") return { ...none, parent: "sessions" }
  if (rest === "/setup") {
    const stage = params.get("stage") || null
    return { parent: "setup", child: stage, leaf: stage ? params.get("step") || null : null }
  }
  if (rest === "/library") {
    const dir = params.get("dir")
    const child = (CLAUDE_DIRS as readonly string[]).includes(dir ?? "") ? dir : null
    return { parent: "library", child, leaf: child ? params.get("item") || null : null }
  }
  if (rest === "/plugins") return { ...none, parent: "plugins", child: params.get("item") || null }
  if (rest === "/machines") return { ...none, parent: "machines", child: params.get("item") || null }
  if (rest === "/apps") return { ...none, parent: "apps", child: params.get("item") || null }
  if (rest === "/settings") return { ...none, parent: "settings", child: params.get("view") === "general" ? "general" : null }
  if (rest in SETTINGS_ROUTES) return { ...none, parent: "settings", child: SETTINGS_ROUTES[rest] }
  return none
}

const PARENT_HREF: Record<ParentId, string> = {
  sessions: "/sessions/",
  plugins: "/plugins/",
  apps: "/apps/",
  library: "/library/",
  machines: "/machines/",
  setup: "/setup/",
  settings: "/settings/",
}

/** One level up from the address: the leaf goes to its child, a child to its parent, a parent to the chat. */
export function backHref(pathname: string, params: Params): string {
  const { lang } = splitPath(pathname)
  const { parent, child, leaf } = trailOf(pathname, params)
  const home = `/${lang}/`
  if (!parent) return home
  if (leaf && child) {
    if (parent === "setup") return `/${lang}/setup/?stage=${encodeURIComponent(child)}`
    if (parent === "library") return `/${lang}/library/?dir=${encodeURIComponent(child)}`
  }
  if (child) return `/${lang}${PARENT_HREF[parent]}`
  return home
}

const DONE: StepStatus[] = ["pass", "fixed", "updated"]

/** `done/total` per stage; a step with status `skip` counts in neither number. */
export function stageCounts(steps: { stage: string; status: StepStatus }[]): Record<string, { done: number; total: number }> {
  const out: Record<string, { done: number; total: number }> = {}
  for (const s of steps) {
    if (s.status === "skip") continue
    const c = (out[s.stage] ??= { done: 0, total: 0 })
    c.total += 1
    if (DONE.includes(s.status)) c.done += 1
  }
  return out
}

export interface SettingsChild {
  id: string
  label: Copy
  hint: Copy
  href: (lang: string) => string
  /** Only inside the app. */
  app?: boolean
  /** Only when the profile allows the engine. */
  engine?: boolean
}

/** Settings' children: the same five everywhere (sidebar, footer, cards). */
export const SETTINGS_CHILDREN: SettingsChild[] = [
  { id: "general", label: copy.general, hint: copy.generalHint, href: (l) => `/${l}/settings/?view=general` },
  { id: "model", label: copy.model, hint: copy.modelHint, href: (l) => `/${l}/model/`, app: true },
  { id: "config", label: copy.configuration, hint: copy.configHint, href: (l) => `/${l}/config/`, app: true },
  { id: "health", label: copy.healthUpdates, hint: copy.healthHint, href: (l) => `/${l}/health/`, app: true },
  { id: "engine", label: copy.engine, hint: copy.engineHint, href: (l) => `/${l}/engine/`, app: true, engine: true },
]

export function settingsChildren(inApp: boolean, engine: boolean): SettingsChild[] {
  return SETTINGS_CHILDREN.filter((c) => (!c.app || inApp) && (!c.engine || engine))
}

export interface TreeInput {
  isAr: boolean
  inApp: boolean
  /** Whether the profile allows the engine. */
  engine: boolean
  stages: Stage[]
  steps: { stage: string; status: StepStatus }[]
}

/** The tree, in the order a person works: Sessions, Plugins, Apps, Library, Machines, Setup, Settings. Setup's children are the readiness stages Apps does not own, each with its `done/total`. */
export function buildTree({ isAr, inApp, engine, stages, steps }: TreeInput): NavNode[] {
  const counts = stageCounts(steps)
  return [
    { id: "sessions", label: copy.sessions, href: (l) => `/${l}/sessions/` },
    {
      id: "plugins",
      label: copy.plugins,
      href: (l) => `/${l}/plugins/`,
      children: CONNECTORS.map((c) => ({ id: c.id, label: c.name, href: (l: string) => `/${l}/plugins/?item=${encodeURIComponent(c.id)}` })),
    },
    {
      id: "apps",
      label: copy.apps,
      href: (l) => `/${l}/apps/`,
      children: APPS.map((a) => ({ id: a.id, label: a.name, href: (l: string) => `/${l}/apps/?item=${encodeURIComponent(a.id)}` })),
    },
    {
      id: "library",
      label: copy.library,
      href: (l) => `/${l}/library/`,
      children: CLAUDE_DIRS.map((d) => ({ id: d, label: d, href: (l: string) => `/${l}/library/?dir=${d}` })),
    },
    // Its children are the machines on the network, known only to the page: the one open on the
    // address is added by the sidebar (like an open Setup stage the catalog does not list).
    { id: "machines", label: copy.machines, href: (l) => `/${l}/machines/`, children: [] },
    {
      id: "setup",
      label: copy.setup,
      href: (l) => `/${l}/setup/`,
      children: stages
        .filter((s) => !APP_STAGES.includes(s.id))
        .map((s) => ({
          id: s.id,
          label: (s.title && (isAr ? s.title.ar : s.title.en)) || s.id,
          href: (l: string) => `/${l}/setup/?stage=${encodeURIComponent(s.id)}`,
          hint: counts[s.id] ? `${counts[s.id].done}/${counts[s.id].total}` : undefined,
        })),
    },
    {
      id: "settings",
      label: copy.settings,
      href: (l) => `/${l}/settings/`,
      children: settingsChildren(inApp, engine).map((c) => ({ id: c.id, label: c.label, href: c.href })),
    },
  ]
}
