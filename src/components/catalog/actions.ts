// catalog — the TS face of rust.rs. Not server actions: in machine these call the local Rust
// plugin over Tauri IPC. In a plain browser the catalog is read-only, served from the static
// registry that `pnpm kun:registry` builds; install and uninstall need the machine app.
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { CatalogItem, Entry, EntryState, Owner } from "./types"

export type { CatalogItem, Entry, EntryState, Owner }

/** True inside the machine app, where install and uninstall work. */
export function canInstall(): boolean {
  return isTauri()
}

/** Every registry item. In machine each carries its status here; in a browser all are "available". */
export async function list(): Promise<CatalogItem[]> {
  if (isTauri()) return invoke<CatalogItem[]>("plugin:catalog|list")
  const res = await fetch("/r/kun/index.json")
  if (!res.ok) throw new Error(`registry ${res.status}`)
  const reg = (await res.json()) as { items: CatalogItem[] }
  return reg.items.map((i) => ({ ...i, status: "available", installed_version: null }))
}

/** Install an item and its dependencies. Resolves to the ids written. */
export async function install(name: string): Promise<string[]> {
  if (!isTauri()) throw new Error("install needs the machine app")
  return invoke<string[]>("plugin:catalog|install", { name })
}

/** Remove only the files this catalog installed for the item. */
export async function uninstall(name: string): Promise<void> {
  if (!isTauri()) throw new Error("uninstall needs the machine app")
  return invoke<void>("plugin:catalog|uninstall", { name })
}

/**
 * The real ~/.claude (skills, agents, rules, hooks, packs) merged with the registry. Outside the
 * app there is no disk to read: it is the static registry, every unit "available" and nothing
 * actionable.
 */
export async function tree(): Promise<Entry[]> {
  if (isTauri()) return invoke<Entry[]>("plugin:catalog|tree")
  const items = await list()
  return items
    .map((i): Entry | null => {
      const dir = DIRS[i.id.split(":")[0]]
      if (!dir) return null
      return {
        id: i.id,
        dir,
        name: i.name,
        title: i.title,
        description: i.description,
        ...(i.title_ar === undefined ? {} : { title_ar: i.title_ar }),
        ...(i.description_ar === undefined ? {} : { description_ar: i.description_ar }),
        state: "available",
        owner: "kun",
        files: i.files.map((f) => f.target),
        dependencies: i.dependencies,
        keywords: i.keywords,
        version: i.version,
        can_install: false,
        can_uninstall: false,
        can_toggle: false,
      }
    })
    .filter((e): e is Entry => e !== null)
    .sort((a, b) => a.dir.localeCompare(b.dir) || a.name.localeCompare(b.name))
}

const DIRS: Record<string, Entry["dir"]> = {
  skill: "skills",
  agent: "agents",
  rule: "rules",
  hook: "hooks",
  pack: "departments",
}

/** Move a skill, agent or rule aside without deleting it (hooks and packs cannot be). */
export async function disable(id: string): Promise<void> {
  if (!isTauri()) throw new Error("disable needs the machine app")
  return invoke<void>("plugin:catalog|disable", { id })
}

/** Move a disabled skill, agent or rule back. */
export async function enable(id: string): Promise<void> {
  if (!isTauri()) throw new Error("enable needs the machine app")
  return invoke<void>("plugin:catalog|enable", { id })
}
