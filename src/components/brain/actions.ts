// brain — the TS face of rust.rs (Tauri IPC). Keys cross only inward (`setSecret`); the page learns
// "set / not set". Outside the machine app every read is empty and every write throws.
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { Brain, BrainView, ProviderRow, ProvidersView, RouteStatus, TestResult } from "./types"

export type { Brain, BrainView, ProviderRow, ProvidersView, RouteStatus, TestResult }

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

/**
 * The free route a session fell back to because Claude was unavailable (usage limit, not signed in,
 * no subscription), or null when it runs on its chosen route. The chip shows "(fallback)" from it.
 */
export async function brainFallbackState(session: string): Promise<Brain | null> {
  if (!isTauri()) return null
  return invoke<Brain | null>("plugin:brain|brain_fallback_state", { session })
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

/** Creates the key OmniRoute runs need (runs kun's `omni.mjs mint-key`, starting the service first if it is stopped). */
export async function brainMintKey(): Promise<BrainView> {
  if (!isTauri()) throw new Error("the model needs the machine app")
  return invoke<BrainView>("plugin:brain|brain_mint_key")
}

/** kun's providers catalog merged with the harvest ledger; null outside the app. */
export async function providersStatus(): Promise<ProvidersView | null> {
  if (!isTauri()) return null
  return invoke<ProvidersView>("plugin:brain|providers_status")
}

/**
 * Saves a pasted key (and the account id where the provider has one), connects it to the gateway and
 * rebuilds the combos. The key travels once and is never sent back; the view only says "connected".
 */
export async function providersSave(id: string, key: string, account?: string): Promise<ProvidersView> {
  if (!isTauri()) throw new Error("providers need the machine app")
  return invoke<ProvidersView>("plugin:brain|providers_save", { id, key, account: account?.trim() ? account : null })
}

/** One upstream test of a connected provider. */
export async function providersTest(id: string): Promise<TestResult> {
  if (!isTauri()) throw new Error("providers need the machine app")
  return invoke<TestResult>("plugin:brain|providers_test", { id })
}

/** Opens the provider's sign-up page in the default browser. */
export async function providersOpen(id: string): Promise<void> {
  if (!isTauri()) throw new Error("providers need the machine app")
  return invoke<void>("plugin:brain|providers_open", { id })
}

/** Installs the gateway service, connects every enabled provider with a key and builds the combos. */
export async function providersSetup(): Promise<ProvidersView> {
  if (!isTauri()) throw new Error("providers need the machine app")
  return invoke<ProvidersView>("plugin:brain|providers_setup")
}
