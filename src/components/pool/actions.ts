// pool — the TS face of rust.rs (IPC to the local Rust plugin). Empty outside the machine app.
import { Channel, invoke, isTauri } from "@tauri-apps/api/core"

import type { DonorStatus, PoolJob, PoolList, UploadEvent } from "./types"

export type { DonorStatus, PoolJob, PoolList, UploadEvent }

/** My jobs and (when sharing) the queue; null outside the app. */
export async function poolJobs(): Promise<PoolList | null> {
  if (!isTauri()) return null
  return invoke<PoolList>("plugin:pool|pool_jobs")
}

/** Creates the job, uploads the files (progress on `onEvent`), and queues it. Rejects with one line. */
export async function poolSubmit(
  kind: string,
  params: Record<string, unknown>,
  target: string | null,
  paths: string[],
  onEvent?: (e: UploadEvent) => void,
): Promise<PoolJob> {
  if (!isTauri()) throw new Error("jobs need the machine app")
  const channel = new Channel<UploadEvent>()
  channel.onmessage = (e) => onEvent?.(e)
  return invoke<PoolJob>("plugin:pool|pool_submit", { kind, params, target, paths, onEvent: channel })
}

export async function poolCancel(id: string): Promise<void> {
  if (!isTauri()) return
  await invoke("plugin:pool|pool_cancel", { id })
}

/** Saves a done job's files to ~/Downloads/kun-pool/<id> and reveals them; resolves with the folder. */
export async function poolDownload(id: string): Promise<string> {
  if (!isTauri()) throw new Error("downloads need the machine app")
  return invoke<string>("plugin:pool|pool_download", { id })
}

/** What this machine's donor side is doing; null outside the app. */
export async function poolStatus(): Promise<DonorStatus | null> {
  if (!isTauri()) return null
  return invoke<DonorStatus>("plugin:pool|pool_status")
}
