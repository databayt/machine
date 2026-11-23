// chat — the machine-read blocks an agent turn may end with: `kun-questions`, `kun-plan`, `kun-spec`.
// One reader for all three (the block must be the last thing in the turn). Pure: no React, no IPC
// (node --test src/components/chat/block.test.ts).
import { type AskQuestion, parseQuestionsJson } from "./ask-pure.ts"

/** The line's 14 stages, in order (the same list as `production/pure.ts`; kept here so chat has no dependency on it). */
export const PLAN_STAGES = ["request", "research", "brief", "script", "storyboard", "assets", "visual", "audio", "edit", "render", "qa", "approval", "cdn", "publish"] as const

const FENCE = "```"
export const BLOCKS = ["kun-questions", "kun-plan", "kun-spec"] as const

type Obj = Record<string, unknown>
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v)
const str = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v.trim() : null)

/**
 * A TRAILING fenced block with this info string. `found` is true when it is the last thing in the text, its body
 * holds no fence and its body is JSON; `prose` is then the text without it. In every other case the text comes
 * back exactly as it was (`found: false`, `json: null`).
 */
export function trailingBlock(text: string, info: string): { prose: string; json: unknown; found: boolean } {
  const same = { prose: text, json: null as unknown, found: false }
  const trimmed = text.trimEnd()
  if (!trimmed.endsWith(FENCE)) return same
  const start = trimmed.lastIndexOf(FENCE + info)
  if (start < 0) return same
  const bodyStart = trimmed.indexOf("\n", start)
  const bodyEnd = trimmed.length - FENCE.length
  if (bodyStart < 0 || bodyStart >= bodyEnd) return same
  const body = trimmed.slice(bodyStart + 1, bodyEnd)
  if (body.includes(FENCE)) return same
  let json: unknown
  try {
    json = JSON.parse(body)
  } catch {
    return same
  }
  return { prose: trimmed.slice(0, start).trimEnd(), json, found: true }
}

/** While a turn streams: its text cut at the last opener of any block (one still being written, or just finished). */
export function hideBlocks(text: string): string {
  let at = -1
  for (const b of BLOCKS) at = Math.max(at, text.lastIndexOf(FENCE + b))
  return at < 0 ? text : text.slice(0, at).trimEnd()
}

/** The plan a `kun-plan` block carries: the film's fields, the cost lines and, per stage, what was decided. */
export interface PlanView {
  film: Obj | null
  cost: string[]
  stages: Record<string, Obj>
}

/** Tolerant: keeps `film`, `cost` (strings) and the known stages; null when nothing usable is left. */
export function parsePlan(json: unknown): PlanView | null {
  if (!isObj(json)) return null
  const film = isObj(json.film) && Object.keys(json.film).length ? json.film : null
  const cost = Array.isArray(json.cost) ? json.cost.filter((c): c is string => typeof c === "string" && !!c.trim()).map((c) => c.trim()) : []
  const stages: Record<string, Obj> = {}
  if (isObj(json.stages)) for (const k of PLAN_STAGES) if (isObj(json.stages[k])) stages[k] = json.stages[k] as Obj
  if (!film && cost.length === 0 && Object.keys(stages).length === 0) return null
  return { film, cost, stages }
}

/** One stage's decisions, as a `kun-spec` block writes them. */
export interface SpecView {
  stage: string
  decisions: Obj
  tool: string | null
  notes: string | null
}

/** The stage must be one of the 14 and `decisions` (when present) an object; `tool` and `notes` are strings or left out. */
export function parseSpec(json: unknown): SpecView | null {
  if (!isObj(json)) return null
  const stage = str(json.stage)
  if (!stage || !(PLAN_STAGES as readonly string[]).includes(stage)) return null
  if (json.decisions !== undefined && !isObj(json.decisions)) return null
  return { stage, decisions: (json.decisions as Obj | undefined) ?? {}, tool: str(json.tool), notes: typeof json.notes === "string" ? json.notes : null }
}

export interface TurnBlocks {
  prose: string
  questions: AskQuestion[] | null
  plan: PlanView | null
  spec: SpecView | null
}

/** A finished turn's trailing block, tried as questions, then plan, then spec. */
export function parseTurn(text: string): TurnBlocks {
  const none = { questions: null, plan: null, spec: null }
  const q = trailingBlock(text, "kun-questions")
  if (q.found) return { ...none, prose: q.prose, questions: parseQuestionsJson(q.json) }
  const p = trailingBlock(text, "kun-plan")
  if (p.found) return { ...none, prose: p.prose, plan: parsePlan(p.json) }
  const s = trailingBlock(text, "kun-spec")
  if (s.found) return { ...none, prose: s.prose, spec: parseSpec(s.json) }
  return { ...none, prose: text }
}

/** What Accept saves to a file: the plan block's parts (never a null film) with the tab picks, the `--plan` shape of `produce lock`. */
export function planFile(plan: PlanView | null | undefined, picks: Record<string, string>): Obj {
  const out: Obj = {}
  if (plan?.film) out.film = plan.film
  if (plan && Object.keys(plan.stages).length) out.stages = plan.stages
  if (plan && plan.cost.length) out.cost = plan.cost
  out.picks = picks
  return out
}

/** A spec as `produce lock --spec` reads it: the `kun-spec` block's own shape. */
export function specFile(spec: SpecView): Obj {
  return { stage: spec.stage, decisions: spec.decisions, ...(spec.tool ? { tool: spec.tool } : {}), ...(spec.notes ? { notes: spec.notes } : {}) }
}
