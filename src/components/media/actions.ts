// media — the TS face of rust.rs. IPC to the local Rust plugin, plus the native file picker.
import { invoke, isTauri } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"

import type { MediaItem } from "./types"

/** The images and videos of a repository that can be attached to a message. Empty outside the app. */
export async function mediaLibrary(repo: string): Promise<MediaItem[]> {
  if (!isTauri()) return []
  return invoke<MediaItem[]>("plugin:media|media_library", { repo })
}

/** Opens the native picker for images, videos and PDFs; absolute paths, empty when cancelled. */
export async function pickLocalFiles(title: string): Promise<string[]> {
  if (!isTauri()) return []
  const picked = await open({
    multiple: true,
    title,
    filters: [
      {
        name: "Media",
        extensions: ["png", "jpg", "jpeg", "webp", "gif", "avif", "mp4", "mov", "webm", "m4v", "pdf"],
      },
    ],
  })
  return picked ?? []
}
