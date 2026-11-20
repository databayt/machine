// tools — the TS face of rust.rs. In a plain browser it degrades to "not available".
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { ToolStatus } from "./types"

/** Production tools installed on this computer. Empty outside the machine app. */
export async function tools(): Promise<ToolStatus[]> {
  if (!isTauri()) return []
  return invoke<ToolStatus[]>("plugin:tools|tools")
}
