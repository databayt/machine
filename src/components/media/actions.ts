// media — the TS face of rust.rs. IPC to the local Rust plugin, plus the native file picker.
import { invoke, isTauri } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"

import { PICK_EXTENSIONS, type PickKind } from "./kinds"
import type { MediaItem } from "./types"

/** The images and videos of a repository that can be attached to a message. Empty outside the app. */
export async function mediaLibrary(repo: string): Promise<MediaItem[]> {
  if (!isTauri()) return []
  return invoke<MediaItem[]>("plugin:media|media_library", { repo })
}

/** Opens the native picker for one kind of file; absolute paths, empty when cancelled. */
export async function pickLocalFiles(title: string, kind: PickKind = "any"): Promise<string[]> {
  if (!isTauri()) return []
  const picked = await open({
    multiple: true,
    title,
    filters: kind === "any" ? undefined : [{ name: kind, extensions: PICK_EXTENSIONS[kind] }],
  })
  return picked ?? []
}

/** Opens the native picker for a folder; its absolute path, null when cancelled. */
export async function pickLocalFolder(title: string): Promise<string | null> {
  if (!isTauri()) return null
  const picked = await open({ directory: true, multiple: false, title })
  return typeof picked === "string" ? picked : null
}
