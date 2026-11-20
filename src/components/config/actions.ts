// config — the TS face of rust.rs (Tauri IPC). Secrets never cross this file: `env_present` is a
// boolean and loading them runs a script in Terminal.
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { ConfigView, McpView, WriteOutcome } from "./types"

export type { ConfigView, McpView, WriteOutcome }

function need() {
  if (!isTauri()) throw new Error("configuration needs the machine app")
}

export async function configLoad(): Promise<ConfigView | null> {
  if (!isTauri()) return null
  return invoke<ConfigView>("plugin:config|config_load")
}

export async function setNotify(text: string): Promise<WriteOutcome> {
  need()
  return invoke<WriteOutcome>("plugin:config|config_set_notify", { text })
}

/** `value` is JSON text. */
export async function setSetting(key: string, value: string): Promise<WriteOutcome> {
  need()
  return invoke<WriteOutcome>("plugin:config|config_set_setting", { key, value })
}

export async function removeSetting(key: string): Promise<WriteOutcome> {
  need()
  return invoke<WriteOutcome>("plugin:config|config_remove_setting", { key })
}

/** Slow: `claude mcp list` health-checks every server. */
export async function mcpList(): Promise<McpView> {
  need()
  return invoke<McpView>("plugin:config|config_mcp_list")
}

export async function mcpRemove(name: string): Promise<WriteOutcome> {
  need()
  return invoke<WriteOutcome>("plugin:config|config_mcp_remove", { name })
}

export async function mcpAdd(name: string): Promise<WriteOutcome> {
  need()
  return invoke<WriteOutcome>("plugin:config|config_mcp_add", { name })
}

/** Opens Terminal running kun's secrets.sh with the gist id. */
export async function loadSecrets(gistId: string): Promise<void> {
  need()
  return invoke<void>("plugin:config|config_load_secrets", { gistId })
}
