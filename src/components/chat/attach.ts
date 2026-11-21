// chat — what the composer carries besides text: attachments and the references block they become.
import type { MediaItem } from "../media/types"

export interface Attachment {
  /** Unique within the composer: the library id, or the file path. */
  key: string
  kind: "image" | "video" | "file"
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

export const fromPath = (path: string): Attachment => ({
  key: path,
  kind: "file",
  title: path.split(/[\\/]/).pop() || path,
  ref: path,
})

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
