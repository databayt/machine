// terminal — the TS face of rust.rs (Tauri IPC). Sessions live in Rust; these only ask and listen.
// A pty needs the machine app; in a browser these say so.
import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

import type { Attached, TerminalChunk, TerminalLane, TerminalList } from "./types"

export type { Attached, SessionInfo, TerminalChunk, TerminalLane, TerminalList } from "./types"

const P = "plugin:terminal|"

/** The launchers the picker offers. Empty outside the app. */
export async function terminalLanes(): Promise<TerminalLane[]> {
  if (!isTauri()) return []
  return invoke<TerminalLane[]>(`${P}terminal_lanes`)
}

/** The sessions in tab order and the active id. */
export async function terminalList(): Promise<TerminalList> {
  if (!isTauri()) return { sessions: [], active: null }
  return invoke<TerminalList>(`${P}terminal_list`)
}

/** The list changed (open, close, rename, active, exit). Resolves with the function that stops listening. */
export function onTerminalSessions(fn: (list: TerminalList) => void): Promise<() => void> {
  return listen<TerminalList>("terminal://sessions", (e) => fn(e.payload))
}

/**
 * Opens a shell: `lane` null is the person's own login zsh ("Local"); a launcher id runs that
 * launcher inside a shell. Resolves with the new session's id; it is also the active one.
 */
export async function terminalOpen(lane: string | null, cols: number, rows: number, cwd?: string): Promise<string> {
  if (!isTauri()) throw new Error("the terminal needs the machine app")
  return invoke<string>(`${P}terminal_open`, { lane, cwd: cwd ?? null, cols, rows, id: null })
}

/** The scrollback of a session (base64) and how many bytes it covers. */
export async function terminalAttach(id: string): Promise<Attached> {
  return invoke<Attached>(`${P}terminal_attach`, { id })
}

export async function terminalWrite(id: string, data: string): Promise<void> {
  await invoke(`${P}terminal_write`, { id, data })
}

export async function terminalResize(id: string, cols: number, rows: number): Promise<void> {
  await invoke(`${P}terminal_resize`, { id, cols, rows })
}

/** Kills the process and frees the session; closing one that is already gone is fine. */
export async function terminalClose(id: string): Promise<void> {
  await invoke(`${P}terminal_close`, { id })
}

export async function terminalCloseOthers(id: string): Promise<void> {
  await invoke(`${P}terminal_close_others`, { id })
}

/** Empty name restores the default. */
export async function terminalRename(id: string, name: string): Promise<void> {
  await invoke(`${P}terminal_rename`, { id, name })
}

export async function terminalSetActive(id: string): Promise<void> {
  await invoke(`${P}terminal_set_active`, { id })
}

/** The app window goes native full screen, or leaves it (a no-op outside the app). */
export async function terminalFullscreen(on: boolean): Promise<void> {
  if (isTauri()) await invoke(`${P}terminal_fullscreen`, { on })
}

/** Raw bytes of a base64 chunk. */
export function decode(chunk: string): Uint8Array {
  const bin = atob(chunk)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}

/** Live output of session `id`. Each chunk carries where it ends, so a replay can skip what it already has. */
export function onTerminalData(id: string, fn: (chunk: TerminalChunk) => void): Promise<() => void> {
  return listen<TerminalChunk>(`terminal://data/${id}`, (e) => fn(e.payload))
}

/** Session `id` ended, with its exit code when there is one. */
export function onTerminalExit(id: string, fn: (code: number | null) => void): Promise<() => void> {
  return listen<number | null>(`terminal://exit/${id}`, (e) => fn(e.payload))
}
