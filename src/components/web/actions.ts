// web — the TS face of rust.rs (IPC to the local Rust plugin). Empty or null outside the machine app.
// The seats are native webviews laid over the Brainstorm page's content box; these only ask Rust to
// place them and read what the page says. The sites themselves get no IPC.
import { Channel, invoke, isTauri } from "@tauri-apps/api/core"

import type { Event } from "../driver/types"
import type { Rect, SeatList, SeatStatus, Turn } from "./types"

export type { Rect, Seat, SeatInfo, SeatList, SeatStatus, Turn } from "./types"

const P = "plugin:web|"

/** kun's seats (ChatGPT, DeepSeek, GLM, Gemini) and which are open. `missing` when kun has no adapters here. */
export async function webSeats(): Promise<SeatList> {
  if (!isTauri()) return { seats: [], missing: false }
  return invoke<SeatList>(`${P}web_seats`)
}

/** Shows the seat over `rect` (window pixels) and hides the others; call again to move or resize it. */
export async function webOpen(id: string, rect: Rect): Promise<void> {
  if (isTauri()) await invoke(`${P}web_open`, { id, rect })
}

/** Hides every seat (the page was left, or a menu is over it). Logins and conversations stay. */
export async function webHide(): Promise<void> {
  if (isTauri()) await invoke(`${P}web_hide`)
}

/** Destroys the seat's webview; its login stays on disk. */
export async function webClose(id: string): Promise<void> {
  if (isTauri()) await invoke(`${P}web_close`, { id })
}

/** What the adapter says about the open seat: ready, needs a login, limit message, busy. */
export async function webStatus(id: string): Promise<SeatStatus> {
  return invoke<SeatStatus>(`${P}web_status`, { id })
}

/** The conversation on the seat's page, oldest turn first. */
export async function webTranscript(id: string): Promise<Turn[]> {
  return invoke<Turn[]>(`${P}web_transcript`, { id })
}

/**
 * Asks the seat in the background (its webview stays hidden unless it is already showing) and streams the
 * usual driver events: `tool`, then `text` + `done`, or `error` (`limit: …` / `login: …`).
 */
export async function webAsk(id: string, prompt: string, onEvent: (event: Event) => void): Promise<void> {
  if (!isTauri()) throw new Error("not in the machine app")
  const channel = new Channel<Event>()
  channel.onmessage = onEvent
  await invoke(`${P}web_ask`, { id, prompt, channel })
}
