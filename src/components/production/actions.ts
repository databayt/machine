// production — the TS face of rust.rs. IPC to the local Rust plugin; every call degrades to "nothing to show".
import { invoke, isTauri } from "@tauri-apps/api/core"

import { type Brand, type Film, type FilmSummary, parseBrand, parseList, parseStatus } from "./pure"

/** A film's status with the raw shot list and asset manifest beside it (parsed into scenes by `parseScenes`). */
export interface FilmData {
  film: Film
  shotList: unknown
  manifest: unknown
}

/** The films of a repo, newest first. Never rejects: any failure is no films, and the strip stays hidden. */
export async function listFilms(repo: string | null): Promise<FilmSummary[]> {
  if (!isTauri()) return []
  try {
    return parseList(await invoke<unknown>("plugin:production|production_list", { repo: repo || null }))
  } catch {
    return []
  }
}

/** One film's status, shot list and asset manifest; null on any failure. */
export async function filmData(id: string): Promise<FilmData | null> {
  if (!isTauri()) return null
  try {
    const raw = await invoke<{ status?: unknown; shotList?: unknown; assetManifest?: unknown }>("plugin:production|production_status", { id })
    const film = parseStatus(raw?.status)
    return film ? { film, shotList: raw.shotList ?? null, manifest: raw.assetManifest ?? null } : null
  } catch {
    return null
  }
}

const brands = new Map<string, Promise<Brand | null>>()

/** The brand's on-screen kit, read once per identity (Rust caches it too). */
export function brandOf(identity: string): Promise<Brand | null> {
  if (!isTauri() || !identity) return Promise.resolve(null)
  let p = brands.get(identity)
  if (!p) {
    p = invoke<unknown>("plugin:production|production_brand", { identity }).then(parseBrand, () => null)
    brands.set(identity, p)
  }
  return p
}

const thumbs = new Map<string, Promise<string | null>>()

/** An image on this Mac as a data URL (Rust checks it: an image under the home folder, 1.5 MB at most). */
export function thumbOf(path: string): Promise<string | null> {
  if (!isTauri()) return Promise.resolve(null)
  let p = thumbs.get(path)
  if (!p) {
    p = invoke<string | null>("plugin:production|production_thumb", { path }).catch(() => null)
    thumbs.set(path, p)
  }
  return p
}

/** Which of the paths are files on disk, in order; all true when the check cannot run (nothing is marked missing). */
export async function filesExist(paths: string[]): Promise<boolean[]> {
  if (!isTauri() || paths.length === 0) return paths.map(() => true)
  try {
    return await invoke<boolean[]>("plugin:production|production_exist", { paths })
  } catch {
    return paths.map(() => true)
  }
}
