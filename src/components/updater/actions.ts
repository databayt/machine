// updater — the TS face of rust.rs (Tauri IPC). Outside the app there is nothing to update.
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { UpdateInfo } from "./types"

export type { UpdateInfo }

/** The newer version, or null when this one is current (or outside the app). */
export async function updateCheck(): Promise<UpdateInfo | null> {
  if (!isTauri()) return null
  return invoke<UpdateInfo | null>("plugin:updater|update_check")
}

/** Downloads, verifies, installs and restarts. Only after the person confirms. */
export async function updateInstall(): Promise<void> {
  if (!isTauri()) throw new Error("updates need the machine app")
  return invoke<void>("plugin:updater|update_install")
}
