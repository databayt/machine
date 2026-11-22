// chat — the question card's state machine (Claude Code's AskUserQuestion, no model behind it).
// Pure: no React, no IPC (node --test src/components/chat/questions.test.ts).
import type { Question, QuestionOption } from "../pack/types.ts"

export interface QState {
  questions: readonly Question[]
  /** The tab shown: 0..n-1 a question, n the Submit tab (only when there are several questions). */
  tab: number
  /** The highlighted row of the tab (the options, then "Type something…"). */
  cursor: number
  /** What the person picked or typed, by slot. */
  answers: Record<string, string>
  /** The "Type something…" row is open for typing. */
  typing: boolean
  typed: string
  /** The card is over: sent on to `fill`, or closed keeping the defaults. */
  done: "submit" | "close" | null
}

export type QAction =
  | { type: "move"; delta: number }
  | { type: "digit"; n: number }
  | { type: "enter" }
  | { type: "tab"; delta: number }
  | { type: "esc" }
  | { type: "type"; text: string }
  | { type: "goto"; tab: number }
  | { type: "row"; row: number }

/** Rows of a question: its options, then the free-text row (an `ask` question has only that). */
export function rowCount(q: Question): number {
  return q.options.length + 1
}

/** The tabs: one per question, plus Submit when there are several. */
export function tabCount(s: Pick<QState, "questions">): number {
  return s.questions.length > 1 ? s.questions.length + 1 : 1
}

const onSubmitTab = (s: QState) => s.questions.length > 1 && s.tab >= s.questions.length

/** The value shown as current: the pick, else the station's current value, else its default. */
export function shownValue(q: Question, answers: Record<string, string>): string | null {
  return answers[q.slot] ?? q.current ?? q.default ?? null
}

function startRow(q: Question, answers: Record<string, string>): number {
  const v = shownValue(q, answers)
  const i = v === null ? -1 : q.options.findIndex((o) => o.value === v)
  return i >= 0 ? i : 0
}

export function initState(questions: readonly Question[]): QState {
  const first = questions[0]
  return {
    questions,
    tab: 0,
    cursor: first ? startRow(first, {}) : 0,
    answers: {},
    typing: false,
    typed: "",
    done: null,
  }
}

function enterTab(s: QState, tab: number): QState {
  const n = tabCount(s)
  const t = ((tab % n) + n) % n
  const q = s.questions[t]
  return { ...s, tab: t, cursor: q ? startRow(q, s.answers) : 0, typing: false, typed: "" }
}

/** Records an answer and moves on: the next question, the Submit tab, or done for a single question. */
function answer(s: QState, q: Question, value: string): QState {
  const answers = { ...s.answers, [q.slot]: value }
  if (s.questions.length === 1) return { ...s, answers, typing: false, done: "submit" }
  return enterTab({ ...s, answers }, s.tab + 1)
}

function pick(s: QState, row: number): QState {
  const q = s.questions[s.tab]
  if (!q) return s
  if (row < q.options.length) return answer(s, q, (q.options[row] as QuestionOption).value)
  // The "Type something…" row: open it, keeping what was typed before for this slot.
  return { ...s, cursor: row, typing: true, typed: s.typed || s.answers[q.slot] || "" }
}

/** One input event to the next state. Never mutates. */
export function reduce(s: QState, a: QAction): QState {
  if (s.done) return s
  if (a.type === "type") return s.typing ? { ...s, typed: a.text } : s
  if (a.type === "goto") return enterTab(s, Math.min(Math.max(a.tab, 0), tabCount(s) - 1))
  if (a.type === "tab") return enterTab(s, s.tab + a.delta)
  if (a.type === "esc") return s.typing ? { ...s, typing: false } : { ...s, done: "close" }
  if (onSubmitTab(s)) {
    if (a.type === "enter") return { ...s, done: "submit" }
    return s
  }
  const q = s.questions[s.tab]
  if (!q) return { ...s, done: "close" }
  switch (a.type) {
    case "move": {
      const n = rowCount(q)
      return { ...s, cursor: (((s.cursor + a.delta) % n) + n) % n, typing: false }
    }
    case "digit":
      return a.n >= 1 && a.n <= rowCount(q) ? pick({ ...s, typing: false }, a.n - 1) : s
    case "row":
      return a.row >= 0 && a.row < rowCount(q) ? pick({ ...s, typing: false }, a.row) : s
    case "enter": {
      if (s.typing) {
        const text = s.typed.trim()
        return text ? answer({ ...s, typed: "" }, q, text) : { ...s, typing: false }
      }
      return pick(s, s.cursor)
    }
  }
}

/** The answers to merge over the set of the fill (empty after Esc: the defaults stay). */
export function answersToSet(s: QState): Record<string, string> {
  return s.done === "submit" ? { ...s.answers } : {}
}

/** Every question is answered (the Submit tab shows ✔ for each). */
export function answered(s: QState, i: number): boolean {
  const q = s.questions[i]
  return !!q && s.answers[q.slot] !== undefined
}

const TITLES_AR: Record<string, string> = {
  market: "السوق",
  audience: "الجمهور",
  goal: "الهدف",
  platform: "المنصة",
  format: "الصيغة",
  length: "المدة",
  language: "اللغة",
}

/** A slot's name as a short header: "market" to "Market" (or its Arabic word). */
export function slotTitle(slot: string, isAr: boolean): string {
  if (isAr && TITLES_AR[slot]) return TITLES_AR[slot]
  const t = slot.replace(/[_-]+/g, " ").trim()
  return t.charAt(0).toUpperCase() + t.slice(1)
}

/** What an option reads as, in the language of the app. */
export function optionLabel(o: QuestionOption, isAr: boolean): string {
  return (isAr ? o.label.ar || o.label.en : o.label.en || o.label.ar) || o.value
}

/** The arrow that goes to the next tab: → in LTR, ← in RTL. */
export function tabDelta(key: string, isAr: boolean): number {
  if (key === "ArrowRight") return isAr ? -1 : 1
  if (key === "ArrowLeft") return isAr ? 1 : -1
  return 0
}
