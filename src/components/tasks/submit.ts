// tasks — the pure side of submitting a work order (node --test src/components/tasks/submit.test.ts).
import type { SubmitEvent } from "./types.ts"

/**
 * The composer's text as the receipt takes it: everything is the summary, except one line that starts
 * with `next:` (any case, `:` or `：`), which is the one line for the next person. Empty when there is no summary.
 */
export function splitSummary(text: string): { summary: string; next: string | null } {
  const lines = text.split("\n")
  const at = lines.findIndex((l) => /^\s*next\s*[:：]/i.test(l))
  if (at < 0) return { summary: text.trim(), next: null }
  const next = lines[at]!.replace(/^\s*next\s*[:：]\s*/i, "").trim()
  const summary = lines.filter((_, i) => i !== at).join("\n").trim()
  return { summary, next: next || null }
}

/** Whole-number percent of a file sent; 100 for an empty file. */
export const percent = (sent: number, total: number): number =>
  total <= 0 ? 100 : Math.min(100, Math.floor((sent / total) * 100))

/** The one progress line in the chat. */
export function progressLine(e: SubmitEvent, isAr: boolean): string {
  switch (e.kind) {
    case "prepared":
      return isAr ? `يرفع ${e.count} ملف…` : `Uploading ${e.count} file${e.count === 1 ? "" : "s"}…`
    case "progress": {
      const n = e.count > 1 ? ` (${e.index + 1}/${e.count})` : ""
      return `${isAr ? "يرفع" : "Uploading"} ${e.name}${n} · ${percent(e.sent, e.total)}%`
    }
    case "uploaded":
      return `${isAr ? "تم رفع" : "Uploaded"} ${e.name}`
    case "posting":
      return isAr ? "يرسل الإيصال…" : "Sending the receipt…"
  }
}
