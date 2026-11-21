// pack — the TS face of rust.rs (IPC to the local Rust plugin).
import { Channel, invoke, isTauri } from "@tauri-apps/api/core"

import type { SyncEvent } from "../repos/types"
import type { Health, PackInfo } from "./types"

/** A department's title and quick actions; null outside the machine app. */
export async function packInfo(department: string): Promise<PackInfo | null> {
  if (!isTauri()) return null
  return invoke<PackInfo>("plugin:pack|info", { department })
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
