// chat — the question block a plan run ends its turn with, and the messages that answer it.
// The protocol text lives in kun (`departments/_shared/ask.md`); this is its reader. Pure: no React, no IPC
// (node --test src/components/chat/ask.test.ts).

import { trailingBlock } from "./block-pure.ts"

export interface AskOption {
  label: string
  description: string
  /** The value a standard choice stands for (a stage's question, from `station stage`). */
  value?: string
}

export interface AskQuestion {
  /** The choice key this question sets (a stage's question). */
  key?: string
  question: string
  /** The chip: 12 characters at most. */
  header: string
  multiSelect: boolean
  /** 2 to 4 choices. */
  options: AskOption[]
}

export const MAX_QUESTIONS = 4
export const MIN_OPTIONS = 2
export const MAX_OPTIONS = 4
export const MAX_HEADER = 12

const OPEN = "```kun-questions"

function clean(v: unknown): string {
  return typeof v === "string" ? v.trim() : ""
}

/** The first `MAX_HEADER` characters (by code point, so Arabic and emoji are not cut mid-letter). */
function clip(text: string): string {
  return [...text].slice(0, MAX_HEADER).join("").trim()
}

function validQuestion(raw: unknown): AskQuestion | null {
  if (!raw || typeof raw !== "object") return null
  const r = raw as Record<string, unknown>
  const question = clean(r.question)
  if (!question) return null
  const header = clip(clean(r.header) || question.split(/\s+/)[0] || "")
  if (!header) return null
  if (!Array.isArray(r.options) || r.options.length < MIN_OPTIONS || r.options.length > MAX_OPTIONS) return null
  const options: AskOption[] = []
  for (const o of r.options) {
    if (!o || typeof o !== "object") return null
    const { label, description } = o as Record<string, unknown>
    const l = clean(label)
    if (!l || typeof description !== "string") return null
    const v = clean((o as Record<string, unknown>).value)
    options.push(v ? { label: l, description: description.trim(), value: v } : { label: l, description: description.trim() })
  }
  const key = clean(r.key)
  return { ...(key ? { key } : {}), question, header, multiSelect: r.multiSelect === true, options }
}

/** The valid questions of a block's parsed JSON (1 to 4; an invalid one is dropped, a fifth is ignored), or null. */
export function parseQuestionsJson(json: unknown): AskQuestion[] | null {
  const list = json && typeof json === "object" ? (json as { questions?: unknown }).questions : null
  if (!Array.isArray(list)) return null
  const questions = list
    .map(validQuestion)
    .filter((q): q is AskQuestion => q !== null)
    .slice(0, MAX_QUESTIONS)
  return questions.length ? questions : null
}

/**
 * Finds a TRAILING ```kun-questions block in a finished assistant text. Returns the prose (the text without the
 * block) and the valid questions (1 to 4; an invalid question is dropped, a fifth is ignored, an options list
 * outside 2 to 4 drops its question). A block that is not the last thing in the text is ignored, and a block
 * whose JSON does not parse leaves the text exactly as it came (`questions: null`).
 */
export function parseAsk(text: string): { prose: string; questions: AskQuestion[] | null } {
  const b = trailingBlock(text, "kun-questions")
  if (!b.found) return { prose: text, questions: null }
  return { prose: b.prose, questions: parseQuestionsJson(b.json) }
}

/** While a turn streams: its text without a block that is still being written (or was just finished). */
export function hideBlock(text: string): string {
  const i = text.lastIndexOf(OPEN)
  return i < 0 ? text : text.slice(0, i).trimEnd()
}

/** What the person answered to one question. */
export interface Answered {
  header: string
  question: string
  answer: string
  /** The question's choice key, when it had one. */
  key?: string
  /** The chosen option's value, when it had one. */
  value?: string
  /** The answer was typed on "Type something." */
  typed?: true
}

export const ANSWERS_PREFIX = "User has answered your questions:"
export const ANSWERS_SUFFIX = "You can now continue with the user's answers in mind."
export const DECLINED_MESSAGE = "The user declined to answer the questions; continue with sensible defaults and say which you chose."

const q = (s: string) => s.replace(/"/g, "'").replace(/\s+/g, " ").trim()

/** The message the app sends back into the same session: the text Claude Code itself gives the model. */
export function answersMessage(items: readonly Answered[]): string {
  const pairs = items.map((a) => `"${q(a.question)}"="${q(a.answer)}"`).join(", ")
  return `${ANSWERS_PREFIX} ${pairs}. ${ANSWERS_SUFFIX}`
}

export const isAnswersMessage = (text: string): boolean => text.startsWith(ANSWERS_PREFIX)
export const isDeclinedMessage = (text: string): boolean => text === DECLINED_MESSAGE

/** The pairs back out of an answers message (the question stands in for the header). */
export function parseAnswers(text: string): Answered[] {
  if (!isAnswersMessage(text)) return []
  const body = text.slice(ANSWERS_PREFIX.length)
  const out: Answered[] = []
  for (const m of body.matchAll(/"([^"]*)"="([^"]*)"/g)) out.push({ header: m[1]!, question: m[1]!, answer: m[2]! })
  return out
}
