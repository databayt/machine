// launchers — the TS face of rust.rs (Tauri IPC). Outside the machine app there is no shell to edit.
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { LaunchersView } from "./types"

export type { LauncherRow, LaunchersView } from "./types"

/** Each launcher: its command, whether its program is installed, whether it is in the block; null outside the app. */
export async function launchersStatus(): Promise<LaunchersView | null> {
  if (!isTauri()) return null
  return invoke<LaunchersView>("plugin:launchers|launchers_status")
}

/** Makes these launcher ids the managed block in the shell files. Rejects with one line. */
export async function launchersApply(enabled: string[]): Promise<LaunchersView> {
  if (!isTauri()) throw new Error("launchers need the machine app")
  return invoke<LaunchersView>("plugin:launchers|launchers_apply", { enabled })
}
