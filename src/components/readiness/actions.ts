// readiness — the TS face of rust.rs (Tauri IPC). Outside the machine app every read is empty and
// every write throws: there is no computer to configure.
import { Channel, invoke, isTauri } from "@tauri-apps/api/core"

import type { Event as DriverEvent } from "../driver/types"
import type {
  AgentToolRow,
  FixOutcome,
  Mode,
  ReadinessView,
  RunEvent,
  RunSummary,
} from "./types"

export type { AgentToolRow, FixOutcome, Mode, ReadinessView, RunEvent, RunSummary }

function need() {
  if (!isTauri()) throw new Error("readiness needs the machine app")
}

function stream<T>(onEvent?: (e: T) => void) {
  const channel = new Channel<T>()
  channel.onmessage = (e) => onEvent?.(e)
  return channel
}

/** The catalog's steps for this computer with where each stands; null outside the app. */
export async function readinessState(): Promise<ReadinessView | null> {
  if (!isTauri()) return null
  return invoke<ReadinessView>("plugin:readiness|readiness_state")
}

/** Runs kun's provision script in `mode`; events stream to `onEvent`. One run at a time. */
export async function readinessRun(mode: Mode, onEvent?: (e: RunEvent) => void, only?: string[]): Promise<RunSummary> {
  need()
  return invoke<RunSummary>("plugin:readiness|readiness_run", { mode, only: only ?? null, channel: stream(onEvent) })
}

export async function readinessCancel(): Promise<boolean> {
  need()
  return invoke<boolean>("plugin:readiness|readiness_cancel")
}

/** Does what the step's fix kind says (auto, terminal, link, request). */
export async function readinessFix(id: string, onEvent?: (e: RunEvent) => void): Promise<FixOutcome> {
  need()
  return invoke<FixOutcome>("plugin:readiness|readiness_fix", { id, channel: stream(onEvent) })
}

/** Starts Claude in the kun folder with the provision skill for these steps. */
export async function healWithAgent(ids: string[], onEvent?: (e: DriverEvent) => void): Promise<void> {
  need()
  return invoke<void>("plugin:readiness|heal_with_agent", { ids, channel: stream(onEvent) })
}

export async function agentsTools(): Promise<AgentToolRow[]> {
  if (!isTauri()) return []
  return invoke<AgentToolRow[]>("plugin:readiness|readiness_agents_tools")
}
