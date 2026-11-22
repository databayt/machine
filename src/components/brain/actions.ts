// brain — the TS face of rust.rs (Tauri IPC). Keys cross only inward (`setSecret`); the page learns
// "set / not set". Outside the machine app every read is empty and every write throws.
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { Brain, BrainView, RouteStatus } from "./types"

export type { Brain, BrainView, RouteStatus }

/** The saved route and where each one stands; null outside the app. */
export async function brainStatus(): Promise<BrainView | null> {
  if (!isTauri()) return null
  return invoke<BrainView>("plugin:brain|brain_status")
}

/** Makes `route` the model for runs. Rejects with one line naming what is missing. */
export async function brainApply(route: Brain): Promise<BrainView> {
  if (!isTauri()) throw new Error("the model needs the machine app")
  return invoke<BrainView>("plugin:brain|brain_apply", { route })
}

/** The route routes' keychain names. */
export const KEY_NAME: Record<Exclude<Brain, "claude">, string> = {
  nvidia: "NVIDIA_NIM_API_KEY",
  openrouter: "OPENROUTER_API_KEY",
  omniroute: "OMNIROUTE_API_KEY",
}

/** The readiness step that installs each free route's program. */
export const INSTALL_STEP: Record<Exclude<Brain, "claude">, string> = {
  nvidia: "fcc.cli",
  openrouter: "fcc.cli",
  omniroute: "omniroute.cli",
}
