// chat — one turn of the transcript. Pure types: no React, no IPC.
import type { AskQuestion } from "./ask-pure.ts"

/** One answered question, as the `⎿` block lists it. */
export interface AnswerLine {
  header: string
  question: string
  answer: string
}

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
  /** The turn ended with a `kun-questions` block: the wizard renders under its prose while it is the latest turn. */
  asked?: AskQuestion[] | null
  /** A user turn that carries the wizard's answers: shown as the compact `⎿` block, not a bubble. */
  answers?: AnswerLine[]
  /** A user turn that declined the questions: shown as a quiet line. */
  declined?: boolean
}
