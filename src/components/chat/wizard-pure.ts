// chat — the question wizard's state machine: Claude Code's AskUserQuestion look, fed by a `kun-questions` block.
// Pure: no React, no IPC (node --test src/components/chat/wizard.test.ts).
import type { Answered, AskQuestion } from "./ask-pure.ts"

export interface WState {
  questions: readonly AskQuestion[]
  /** 0..n-1 a question; n is the review screen (not shown for one single-select question: it submits at once). */
  tab: number
  /** The highlighted row: the options, then "Type something.", then "Chat about this". On the review: Submit, Cancel. */
  cursor: number
  /** Single-select: the chosen option per question. */
  picks: (number | null)[]
  /** Multi-select: the ticked options per question. */
  checked: boolean[][]
  /** What was typed on "Type something." per question. */
  typed: string[]
  /** The "Type something." row is open for typing. */
  typing: boolean
  /** Over: submitted (answers go back), cancelled (declined), or "Chat about this" (the person replies in their own words). */
  done: "submit" | "cancel" | "chat" | null
}

export type WAction =
  | { type: "move"; delta: number }
  | { type: "digit"; n: number }
  | { type: "enter" }
  | { type: "space" }
  | { type: "tab"; delta: number }
  | { type: "esc" }
  | { type: "type"; text: string }
  | { type: "goto"; tab: number }
  /** A mouse click: the row is highlighted and chosen. */
  | { type: "row"; row: number }

/** True when answering alone ends the wizard (one single-select question: no review). */
export const direct = (qs: readonly AskQuestion[]): boolean => qs.length === 1 && !qs[0]!.multiSelect

/** The tabs: one per question, plus the review (unless `direct`). */
export function tabCount(s: Pick<WState, "questions">): number {
  return direct(s.questions) ? 1 : s.questions.length + 1
}

export const onReview = (s: WState): boolean => !direct(s.questions) && s.tab >= s.questions.length

/** Rows of a question: its options, "Type something.", "Chat about this". */
export const rowCount = (q: AskQuestion): number => q.options.length + 2
export const typeRow = (q: AskQuestion): number => q.options.length
export const chatRow = (q: AskQuestion): number => q.options.length + 1

/** `cursor` opens the highlight elsewhere than the first option (a stage the agent owns starts on "Chat about this"). */
export function initState(questions: readonly AskQuestion[], cursor?: "chat"): WState {
  return {
    questions,
    tab: 0,
    cursor: cursor === "chat" && questions[0] ? chatRow(questions[0]) : 0,
    picks: questions.map(() => null),
    checked: questions.map((q) => q.options.map(() => false)),
    typed: questions.map(() => ""),
    typing: false,
    done: null,
  }
}

/** The answer text of question `i`, or null when it has none. Multi-select joins ticked labels (and typed text) with ", ". */
export function answerOf(s: WState, i: number): string | null {
  const q = s.questions[i]
  if (!q) return null
  const typed = s.typed[i]!.trim()
  if (q.multiSelect) {
    const labels = q.options.filter((_, j) => s.checked[i]![j]).map((o) => o.label)
    if (typed) labels.push(typed)
    return labels.length ? labels.join(", ") : null
  }
  const p = s.picks[i]
  if (p !== null && p !== undefined) return q.options[p]!.label
  return typed || null
}

export const isAnswered = (s: WState, i: number): boolean => answerOf(s, i) !== null

/** What goes back to the run: one entry per answered question. */
export function collect(s: WState): Answered[] {
  const out: Answered[] = []
  s.questions.forEach((q, i) => {
    const answer = answerOf(s, i)
    if (answer === null) return
    const item: Answered = { header: q.header, question: q.question, answer }
    if (q.key) item.key = q.key
    const p = s.picks[i]
    if (!q.multiSelect && p !== null && p !== undefined) {
      const v = q.options[p]!.value
      if (v) item.value = v
    } else if (s.typed[i]!.trim() && !q.multiSelect) item.typed = true
    out.push(item)
  })
  return out
}

function enterTab(s: WState, tab: number): WState {
  const n = tabCount(s)
  const t = ((tab % n) + n) % n
  const q = s.questions[t]
  // Land on the first ticked or chosen option, else the first row.
  let cursor = 0
  if (q && !q.multiSelect && s.picks[t] !== null && s.picks[t] !== undefined) cursor = s.picks[t]!
  return { ...s, tab: t, cursor, typing: false }
}

/** An answer was given: end the wizard for a single question, else move to the next tab (the review after the last). */
function advance(s: WState): WState {
  if (direct(s.questions)) return { ...s, typing: false, done: "submit" }
  return enterTab(s, s.tab + 1)
}

/** Chooses `row` of the current question (a digit, a click, Enter). */
function choose(s: WState, row: number): WState {
  const q = s.questions[s.tab]
  if (!q) return s
  if (row === chatRow(q)) return { ...s, done: "chat", typing: false }
  if (row === typeRow(q)) return { ...s, cursor: row, typing: true }
  if (row >= q.options.length) return s
  if (q.multiSelect) return toggle({ ...s, cursor: row }, row)
  const picks = s.picks.slice()
  picks[s.tab] = row
  return advance({ ...s, picks, cursor: row })
}

function toggle(s: WState, row: number): WState {
  const q = s.questions[s.tab]
  if (!q?.multiSelect || row >= q.options.length) return s
  const checked = s.checked.map((r) => r.slice())
  checked[s.tab]![row] = !checked[s.tab]![row]
  return { ...s, checked }
}

/** One input event to the next state. Never mutates. */
export function reduce(s: WState, a: WAction): WState {
  if (s.done) return s
  if (a.type === "type") {
    if (!s.typing) return s
    const typed = s.typed.slice()
    typed[s.tab] = a.text
    return { ...s, typed }
  }
  if (a.type === "goto") return enterTab(s, Math.min(Math.max(a.tab, 0), tabCount(s) - 1))
  if (a.type === "tab") return tabCount(s) > 1 ? enterTab(s, s.tab + a.delta) : s
  if (a.type === "esc") return s.typing ? { ...s, typing: false } : { ...s, done: "cancel" }
  if (onReview(s)) return reduceReview(s, a)
  const q = s.questions[s.tab]
  if (!q) return { ...s, done: "cancel" }
  switch (a.type) {
    case "move": {
      const n = rowCount(q)
      return { ...s, cursor: (((s.cursor + a.delta) % n) + n) % n, typing: false }
    }
    case "digit":
      return a.n >= 1 && a.n <= rowCount(q) ? choose({ ...s, typing: false }, a.n - 1) : s
    case "row":
      return a.row >= 0 && a.row < rowCount(q) ? choose({ ...s, typing: false }, a.row) : s
    case "space":
      return s.typing ? s : toggle(s, s.cursor)
    case "enter": {
      if (s.typing) {
        const text = s.typed[s.tab]!.trim()
        if (!text) return { ...s, typing: false }
        if (q.multiSelect) return advance({ ...s, typing: false })
        const picks = s.picks.slice()
        picks[s.tab] = null
        return advance({ ...s, picks, typing: false })
      }
      // Multi-select: Enter on an option moves on (Space ticks); on the other rows it chooses.
      if (q.multiSelect && s.cursor < q.options.length) return advance(s)
      return choose(s, s.cursor)
    }
  }
}

function reduceReview(s: WState, a: WAction): WState {
  switch (a.type) {
    case "move":
      return { ...s, cursor: (((s.cursor + a.delta) % 2) + 2) % 2 }
    case "digit":
      return a.n === 1 ? submit(s) : a.n === 2 ? { ...s, done: "cancel" } : s
    case "row":
      return a.row === 0 ? submit(s) : a.row === 1 ? { ...s, done: "cancel" } : s
    case "enter":
      return s.cursor === 0 ? submit(s) : { ...s, done: "cancel" }
    default:
      return s
  }
}

/** Submit needs at least one answer; with none the review stays (there is nothing to send). */
function submit(s: WState): WState {
  return collect(s).length ? { ...s, done: "submit" } : s
}

/** The arrow that goes to the next tab: → in LTR, ← in RTL. */
export function tabDelta(key: string, isAr: boolean): number {
  if (key === "ArrowRight") return isAr ? -1 : 1
  if (key === "ArrowLeft") return isAr ? 1 : -1
  return 0
}
