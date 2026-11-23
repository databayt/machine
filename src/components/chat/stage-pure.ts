// chat — one stage of a film as a turn written locally (no model): its number, name, lock state, the summary lines
// and, when the stage has standard choices, the questions the wizard asks. Pure: no React, no IPC
// (node --test src/components/chat/stage.test.ts).
import { type Film, gateOf } from "../production/pure.ts"
import { type Answered, type AskQuestion, parseQuestionsJson } from "./ask-pure.ts"

export interface StageView {
  film: string
  stage: string
  n: number
  label: string
  /** Who owns the decisions: "script" (standard choices) or "agent" (a discussion). */
  ask: string
  gate: string | null
  done: boolean
  locked: boolean
  /** Who locked it, when known. */
  lockBy: string | null
  summary: string[]
  questions: AskQuestion[]
  toolLabel: string | null
  /** Built from the film's status alone (an older kun): the Discuss button uses the stage's own formula. */
  fallback?: boolean
}

type Obj = Record<string, unknown>
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v)
const str = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v.trim() : null)

/** `{ en, ar }` or a plain string: the English text (the Video pack is English-only). */
function text(v: unknown): string | null {
  if (isObj(v)) return str(v.en) ?? str(v.ar)
  return str(v)
}

/** The stages whose decisions the agent owns (stages.json `ask: "agent"`); the fallback needs them without the file. */
const AGENT_STAGES = new Set(["brief", "script", "storyboard", "visual"])

/** A tolerant reader of the `station stage` JSON. Null when it carries no film or stage (an `{ error }` answer too). */
export function parseStage(raw: unknown): StageView | null {
  if (!isObj(raw)) return null
  const film = str(raw.film)
  const stage = str(raw.stage)
  if (!film || !stage) return null
  const n = typeof raw.n === "number" ? raw.n : 0
  const lock = isObj(raw.lock) ? raw.lock : null
  const locked = raw.locked === true || !!lock
  return {
    film,
    stage,
    n,
    label: text(raw.label) ?? stage,
    ask: str(raw.ask) ?? "script",
    gate: str(raw.gate),
    done: raw.done === true,
    locked,
    lockBy: str(lock?.by),
    summary: Array.isArray(raw.summary) ? raw.summary.filter((s): s is string => typeof s === "string" && !!s.trim()) : [],
    questions: parseQuestionsJson({ questions: raw.questions }) ?? [],
    toolLabel: str(raw.tool_label),
  }
}

/** A view from the film's status alone, for a kun that has no `station stage`: no questions, so a Discuss button. */
export function stageFromStatus(film: Film, stageName: string): StageView | null {
  const s = film.stages.find((x) => x.name === stageName)
  if (!s) return null
  return {
    film: film.id,
    stage: s.name,
    n: s.n,
    label: s.name[0]!.toUpperCase() + s.name.slice(1),
    ask: AGENT_STAGES.has(s.name) ? "agent" : "script",
    gate: gateOf(film, s.name)?.name ?? null,
    done: s.done,
    locked: s.locked,
    lockBy: null,
    summary: s.file ? [`Output: ${s.file}`] : [],
    questions: [],
    toolLabel: null,
    fallback: true,
  }
}

/** What the wizard collected as the stage's answer: the picked values by choice key, and the text typed on "Type something". */
export function splitAnswers(collected: readonly Answered[]): { set: Record<string, string>; typed: string | null } {
  const set: Record<string, string> = {}
  let typed: string | null = null
  for (const a of collected) {
    if (a.typed) typed = a.answer
    else if (a.key && a.value) set[a.key] = a.value
  }
  return { set, typed }
}

/** Where one stage's discussion is remembered: `film + stage -> session id`. */
export const stageSessionKey = (film: string, stage: string): string => `kun.video.stage-session.${film}.${stage}`

/** The compact heading line: `07 visual · locked by abdout`, or `open`. */
export function stageHeading(v: StageView, words: { locked: string; open: string; by: string }): string {
  const nn = String(v.n).padStart(2, "0")
  const state = v.locked ? (v.lockBy ? `${words.locked} ${words.by} ${v.lockBy}` : words.locked) : words.open
  return `${nn} ${v.stage} · ${state}`
}
