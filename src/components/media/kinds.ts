// media — the file kinds the attach menu offers, by extension. Shared by the picker and the chips.
/** What the native picker offers, by kind. "any" is every file type. */
export type PickKind = "image" | "video" | "audio" | "document" | "any"

export const PICK_EXTENSIONS: Record<Exclude<PickKind, "any">, string[]> = {
  image: ["png", "jpg", "jpeg", "webp", "gif", "avif", "heic", "svg"],
  video: ["mp4", "mov", "webm", "m4v", "mkv"],
  audio: ["mp3", "wav", "m4a", "aac", "ogg", "flac"],
  document: ["pdf", "md", "txt", "docx", "doc", "pptx", "xlsx", "csv", "json"],
}
