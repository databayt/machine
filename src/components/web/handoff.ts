// web — "send to next step": turns a seat's conversation into the prompt the chat opens with. Pure (no
// React, no IPC) so node can test it. There is no GitHub plumbing in the app: an idea or a task note
// becomes a prompt for the same agent that already files issues (the idea skill, `gh`), sent by the person.
import type { Turn } from "./types.ts"

/** The transcript kept in a prompt: the page URL carries it, so the oldest turns go first when it is long. */
export const MAX_TRANSCRIPT_CHARS = 20000

/** `You: …` / `<Seat>: …` turns, oldest first, cut from the front to fit. Empty turns are dropped. */
export function transcriptText(turns: Turn[], seatName: string, max = MAX_TRANSCRIPT_CHARS): string {
  const lines = turns
    .map((t) => ({ who: t.role === "user" ? "You" : seatName, text: t.text.trim() }))
    .filter((t) => t.text)
    .map((t) => `${t.who}: ${t.text}`)
  let text = lines.join("\n\n")
  if (text.length <= max) return text
  text = text.slice(text.length - max)
  const cut = text.indexOf("\n\n")
  return `[earlier turns trimmed]\n\n${cut >= 0 ? text.slice(cut + 2) : text}`
}

export type Next = "idea" | "spec" | "chat"

/** The prompt for each next step. English and fixed: it is an instruction to the agent, not UI copy. */
export function handoffPrompt(next: Next, seatName: string, transcript: string, task?: string): string {
  const body = `\n\n---\n${transcript}\n---`
  switch (next) {
    case "idea":
      return `Capture this brainstorm with ${seatName} as a new idea: a GitHub issue in the right databayt repo (use the idea skill), titled from the idea, with a user story, acceptance criteria and the transcript below as the body. Reply with the link, then stop.${body}`
    case "spec":
      return `Attach this brainstorm with ${seatName} to the spec of ${task ?? "the chosen task"}: comment the transcript on that issue (gh issue comment) under a "Brainstorm" heading, with a short summary of the decisions on top. Reply with the link, then stop.${body}`
    case "chat":
      return `Context from my brainstorm with ${seatName}. Read it, then wait for my next message.${body}`
  }
}
