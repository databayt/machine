// tasks — the TS face of rust.rs (IPC to the local Rust plugin). Null outside the machine app.
import { Channel, invoke, isTauri } from "@tauri-apps/api/core"

import type { Readiness, SubmitEvent, TasksResponse } from "./types"

/** The server's queue and board, plus the local gate verdict for every task id. */
export type TasksView = TasksResponse & {
  readiness: Record<string, { state: Readiness["state"]; missing: string[] }>
}

export async function getTasks(department: string): Promise<TasksView | null> {
  if (!isTauri()) return null
  return invoke<TasksView>("plugin:tasks|tasks", { department })
}

/** Re-checks the gate, claims the task and writes TASK.md. Open the chat with `?task=<id>` after. */
export async function startTask(id: string): Promise<void> {
  if (!isTauri()) return
  await invoke("plugin:tasks|start", { id })
}

export async function releaseTask(id: string): Promise<void> {
  if (!isTauri()) return
  await invoke("plugin:tasks|release", { id })
}

/** Sends the task to review with a summary; the CEO closes the issue. */
export async function finishTask(id: string, summary: string): Promise<void> {
  if (!isTauri()) return
  await invoke("plugin:tasks|done", { id, summary })
}

/**
 * Submits the work: uploads each file to the CDN through kun's presigned links (one progress event per
 * step), then posts the run receipt with the public links and their sha256 hashes. Resolves with the
 * links; rejects with kun's own reason when an upload or the receipt is refused.
 */
export async function submitWork(
  id: string,
  paths: string[],
  summary: string,
  next: string | null,
  onEvent: (event: SubmitEvent) => void,
): Promise<string[]> {
  if (!isTauri()) return []
  const channel = new Channel<SubmitEvent>()
  channel.onmessage = onEvent
  return invoke<string[]>("plugin:tasks|submit_work", { id, paths, summary, next, channel })
}
