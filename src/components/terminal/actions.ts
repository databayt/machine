// terminal — the TS face of rust.rs (Tauri IPC). A pty needs the machine app; in a browser these say so.
import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

import type { TerminalLane } from "./types"

export type { TerminalLane } from "./types"

/** The launchers the picker offers. Empty outside the app. */
export async function terminalLanes(): Promise<TerminalLane[]> {
  if (!isTauri()) return []
  return invoke<TerminalLane[]>("plugin:terminal|terminal_lanes")
}

/**
 * Starts launcher `lane` (an id from kun's launchers list) in a pty and resolves with the id it
 * runs under (the one passed, so the page can listen before the first byte).
 */
export async function terminalOpen(lane: string, cols: number, rows: number, id: string, cwd?: string): Promise<string> {
  if (!isTauri()) throw new Error("the terminal needs the machine app")
  return invoke<string>("plugin:terminal|terminal_open", { lane, cwd: cwd ?? null, cols, rows, id })
}

export async function terminalWrite(id: string, data: string): Promise<void> {
  await invoke("plugin:terminal|terminal_write", { id, data })
}

export async function terminalResize(id: string, cols: number, rows: number): Promise<void> {
  await invoke("plugin:terminal|terminal_resize", { id, cols, rows })
}

/** Kills the process and frees the terminal; closing one that is already gone is fine. */
export async function terminalClose(id: string): Promise<void> {
  await invoke("plugin:terminal|terminal_close", { id })
}

/** Raw bytes of a base64 chunk from `terminal://data/<id>`. */
export function decode(chunk: string): Uint8Array {
  const bin = atob(chunk)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}

/** Output of terminal `id`, as bytes. Resolves with the function that stops listening. */
export function onTerminalData(id: string, fn: (bytes: Uint8Array) => void): Promise<() => void> {
  return listen<string>(`terminal://data/${id}`, (e) => fn(decode(e.payload)))
}

/** Terminal `id` ended, with its exit code when there is one. */
export function onTerminalExit(id: string, fn: (code: number | null) => void): Promise<() => void> {
  return listen<number | null>(`terminal://exit/${id}`, (e) => fn(e.payload))
}
