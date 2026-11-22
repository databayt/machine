// chat — the intake conversation: the first prompt starts the chat, questions come as a reply, then a brief.
// Pure: no React, no IPC (node --test src/components/chat/intake.test.ts).
import type { Question, Words } from "../pack/types.ts"
import { copy } from "./config.ts"
import { modelName } from "./model-pure.ts"
import { optionLabel, shownValue, slotTitle } from "./questions-pure.ts"

/** A chat turn. `kind` marks the local ones (an intake reply, a brief): they are never sent to Claude. */
export interface Message {
  role: "user" | "assistant"
  text: string
  /** Finished tool steps, in order. */
  tools: string[]
  /** The tool running now. */
  live: string | null
  streaming?: boolean
  error?: string
  /** The route the turn ran on (null: none was named). */
  route?: string | null
  /** The turn ran in plan mode: it ends with Accept. */
  plan?: boolean
  /** The `record` of the formula that opened the plan (`issue` or `none`). */
  record?: string | null
  kind?: "intake" | "brief"
  intake?: IntakeState
  brief?: BriefState
}

export type IntakeStatus = "open" | "answered" | "skipped"

/** The questions of one formula, asked inside an assistant turn. `n` is the token that ties them to their brief. */
export interface IntakeState {
  n: number
  dept: string
  id: string
  /** The set the formula was opened with (`new=1` and every other key); the answers merge on top. */
  set: Record<string, string>
  questions: Question[]
  status: IntakeStatus
  answers: Record<string, string>
}

/** The finished brief: what Start sends and Edit puts in the box. */
export interface BriefState {
  id: string
  dept: string
  title: string
  prompt: string
  route: string | null
  record: string
  model: string | null
  effort: string | null
  /** The user's own text, for the fallback when the brief fails. */
  label: string
}

const base = { tools: [] as string[], live: null as string | null }

export function userMessage(text: string): Message {
  return { role: "user", text, ...base }
}

/** The line said before the questions, in the language of the app; a fallback when the formula has none. */
export function introLine(intro: Words | null | undefined, isAr: boolean): string {
  const line = (isAr ? intro?.ar || intro?.en : intro?.en || intro?.ar) ?? ""
  return line.trim() || (isAr ? copy.intakeIntro.ar : copy.intakeIntro.en)
}

export function intakeMessage(
  i: { n: number; dept: string; id: string; set: Record<string, string>; questions: Question[]; intro: Words | null },
  isAr: boolean,
): Message {
  const { intro, ...rest } = i
  return {
    role: "assistant",
    text: introLine(intro, isAr),
    ...base,
    kind: "intake",
    intake: { ...rest, set: { ...i.set }, status: "open", answers: {} },
  }
}

/** "Brief · Intro film · Opus 5.5" (RTL: «الموجز · …»). */
export function briefTitle(label: string, model: string | null | undefined, isAr: boolean): string {
  return [isAr ? copy.briefTitle.ar : copy.briefTitle.en, label, modelName(model)].filter(Boolean).join(" · ")
}

export function briefMessage(b: BriefState): Message {
  return { role: "assistant", text: b.prompt, ...base, kind: "brief", brief: b }
}

/** "Audience: school owners · Market: Saudi Arabia · Cut: 2-minute film"; each value is the pick, else the current, else the default. */
export function answeredSummary(questions: readonly Question[], answers: Record<string, string>, isAr: boolean): string {
  const parts: string[] = []
  for (const q of questions) {
    const v = shownValue(q, answers)
    if (v === null || v === "") continue
    const opt = q.options.find((o) => o.value === v)
    parts.push(`${slotTitle(q.slot, isAr)}: ${opt ? optionLabel(opt, isAr) : v}`)
  }
  return parts.join(" · ")
}

/** The line under the intro once the card is closed: the summary, or what happened instead. */
export function intakeOutcome(i: IntakeState, isAr: boolean): string {
  if (i.status === "skipped") return isAr ? copy.intakeSkipped.ar : copy.intakeSkipped.en
  return answeredSummary(i.questions, i.answers, isAr) || (isAr ? copy.intakeDefaults.ar : copy.intakeDefaults.en)
}

/** The open intake's index, or -1. */
export function openIntake(messages: readonly Message[]): number {
  return messages.findIndex((m) => m.kind === "intake" && m.intake?.status === "open")
}

/** Typing and sending during an open intake dismisses it: the defaults stay, the card shows "skipped". */
export function dismissOpenIntake(messages: Message[]): Message[] {
  if (openIntake(messages) < 0) return messages
  return messages.map((m) => (m.kind === "intake" && m.intake?.status === "open" ? { ...m, intake: { ...m.intake, status: "skipped" } } : m))
}

/** The card is done: it collapses to its summary (the answers kept). */
export function settleIntake(messages: Message[], n: number, answers: Record<string, string>): Message[] {
  return messages.map((m) =>
    m.kind === "intake" && m.intake?.n === n && m.intake.status === "open" ? { ...m, intake: { ...m.intake, status: "answered", answers } } : m,
  )
}

/** A turn that is not a run: a user line, or what the intake made (the questions, the brief). */
function isLocal(m: Message): boolean {
  return m.role === "user" || m.kind !== undefined
}

/**
 * Whether typed text may open an intake on Enter: it must be a sentence (not a slash command), no intake
 * may be open, and no real turn may have happened yet (a reply in a running chat is never matched).
 */
export function canStartFromText(messages: readonly Message[], text: string): boolean {
  const t = text.trim()
  return t.length > 0 && !t.startsWith("/") && openIntake(messages) < 0 && messages.every(isLocal)
}

/** A brief folded to its first lines; `hasMore` says there is a rest to show. */
export function foldBrief(prompt: string, lines = 2): { head: string; hasMore: boolean } {
  const all = prompt.trim().split("\n")
  const kept = all.slice(0, lines)
  return { head: kept.join("\n"), hasMore: all.length > lines }
}
