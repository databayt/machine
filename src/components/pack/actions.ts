// pack — the TS face of rust.rs (IPC to the local Rust plugin).
import { Channel, invoke, isTauri } from "@tauri-apps/api/core"

import type { SyncEvent } from "../repos/types"
import type { Board, Filled, FormulaMeta, Health, LogEvent, Match, PackInfo, Questions } from "./types"

/** A department's title, fallback quick actions, frame and routes; null outside the machine app. */
export async function packInfo(department: string): Promise<PackInfo | null> {
  if (!isTauri()) return null
  return invoke<PackInfo>("plugin:pack|info", { department })
}

/**
 * The station's departure board: at most three options, already ordered. It never rejects: any failure
 * is an empty board, so the home falls back to the pack's quick actions.
 */
export async function board(department: string, repo: string | null, healthLevel: string | null): Promise<Board> {
  const empty: Board = { department, repo, options: [], error: null }
  if (!isTauri()) return empty
  try {
    return await invoke<Board>("plugin:pack|board", { department, repo, healthLevel })
  } catch (e) {
    return { ...empty, error: String(e) }
  }
}

/** One formula filled by the station's script (about 0.1 s, no model); rejects with a message on failure. */
export async function fill(
  department: string,
  id: string,
  repo: string | null,
  set: Record<string, string> = {},
): Promise<Filled> {
  if (!isTauri()) throw new Error("not in the machine app")
  return invoke<Filled>("plugin:pack|fill", { department, id, repo, set })
}

/**
 * The slots a formula still asks (no model, about 0.1 s), for the question card. `set` is the one the fill
 * used, so slots already answered are skipped. Never rejects: any failure is no questions.
 */
export async function questions(
  department: string,
  id: string,
  repo: string | null,
  set: Record<string, string> = {},
): Promise<Questions> {
  const none: Questions = { id, questions: [], intro: null }
  if (!isTauri()) return none
  try {
    return await invoke<Questions>("plugin:pack|questions", { department, id, repo, set })
  } catch {
    return none
  }
}

/** Typed text to a formula id; never rejects, no match is `{ id: null }`. */
export async function matchText(department: string, text: string, repo: string | null): Promise<Match> {
  const none: Match = { id: null, confidence: 0, dept: null, set: {} }
  if (!isTauri()) return none
  try {
    return await invoke<Match>("plugin:pack|match_text", { department, text, repo })
  } catch {
    return none
  }
}

/** The department's formulas (id, labels, intent, icon) for the slash menu; read-only, never rejects. */
export async function formulas(department: string): Promise<FormulaMeta[]> {
  if (!isTauri()) return []
  try {
    return await invoke<FormulaMeta[]>("plugin:pack|formulas", { department })
  } catch {
    return []
  }
}

/** Records a tap or a dismissal for the board's scoring (`~/.kun/machine/station-log.jsonl`); best effort. */
export async function stationLog(department: string, repo: string | null, id: string, event: LogEvent): Promise<void> {
  if (!isTauri()) return
  try {
    await invoke("plugin:pack|station_log", { department, repo, id, event })
  } catch {
    // the log only sharpens the next board
  }
}

/** Departments that ship a pack in this kun checkout. */
export async function departments(): Promise<string[]> {
  if (!isTauri()) return []
  return invoke<string[]>("plugin:pack|departments")
}

/** Clones the profile's repos into ~/Kun/repos (or fast-forwards them); one event per step. Never pushes. */
export async function syncRepos(onEvent: (event: SyncEvent) => void): Promise<void> {
  if (!isTauri()) return
  const channel = new Channel<SyncEvent>()
  channel.onmessage = onEvent
  return invoke<void>("plugin:pack|sync_repos", { channel })
}

/** The department's health light (its pack's `health.check`); null outside the machine app. */
export async function health(department: string): Promise<Health | null> {
  if (!isTauri()) return null
  return invoke<Health>("plugin:pack|health", { department })
}

/** Runs the pack's `health.fix`, then the check again. */
export async function healthFix(department: string): Promise<Health | null> {
  if (!isTauri()) return null
  return invoke<Health>("plugin:pack|health_fix", { department })
}
