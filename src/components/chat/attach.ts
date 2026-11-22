// chat — what the composer carries besides text: attachments and the references block they become.
import { PICK_EXTENSIONS } from "../media/kinds"
import type { MediaItem } from "../media/types"

export interface Attachment {
  /** Unique within the composer: the library id, or the file path. */
  key: string
  kind: "image" | "video" | "audio" | "folder" | "link" | "file"
  title: string
  /** The CDN url or the absolute local path, as the message cites it. */
  ref: string
  /** A 40px thumbnail for library items. */
  thumb?: string
}

export const itemTitle = (i: MediaItem, isAr: boolean): string =>
  (isAr ? i.title_ar || i.title_en : i.title_en || i.title_ar) || i.id

export const fromItem = (i: MediaItem, isAr: boolean): Attachment => ({
  key: i.id,
  kind: i.kind === "video" ? "video" : "image",
  title: itemTitle(i, isAr),
  ref: i.url,
  thumb: i.thumb || undefined,
})

const kindOf = (path: string): Attachment["kind"] => {
  const ext = path.split(".").pop()?.toLowerCase() ?? ""
  if (PICK_EXTENSIONS.image.includes(ext)) return "image"
  if (PICK_EXTENSIONS.video.includes(ext)) return "video"
  if (PICK_EXTENSIONS.audio.includes(ext)) return "audio"
  return "file"
}

export const fromPath = (path: string): Attachment => ({
  key: path,
  kind: kindOf(path),
  title: path.split(/[\\/]/).pop() || path,
  ref: path,
})

export const fromFolder = (path: string): Attachment => ({
  key: path,
  kind: "folder",
  title: path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path,
  ref: path,
})

/** A pasted web address; null when it is not one. */
export function fromLink(raw: string): Attachment | null {
  const value = raw.trim()
  try {
    const url = new URL(/^[a-z]+:\/\//i.test(value) ? value : `https://${value}`)
    if (!/^https?:$/.test(url.protocol) || !url.hostname.includes(".")) return null
    return { key: url.href, kind: "link", title: url.hostname + (url.pathname === "/" ? "" : url.pathname), ref: url.href }
  } catch {
    return null
  }
}

/** The text plus a trailing references block; the text alone when nothing is attached. */
export function withReferences(text: string, items: Attachment[]): string {
  if (!items.length) return text
  const rows = items.map((a) => `- ${a.title} — ${a.ref}`).join("\n")
  return `${text}\n\nمراجع / References:\n${rows}`
}

/** Adds `more`, skipping keys already attached. */
export const mergeAttachments = (have: Attachment[], more: Attachment[]): Attachment[] => [
  ...have,
  ...more.filter((m) => !have.some((h) => h.key === m.key)),
]
