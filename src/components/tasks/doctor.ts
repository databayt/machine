// tasks — the doctor gate as the task cards see it. Pure, so node can test it.

/** The gate steps behind `doctor:<step>` reasons, in order. */
export function doctorSteps(missing: string[]): string[] {
  return missing.filter((m) => m.startsWith("doctor:")).map((m) => m.slice("doctor:".length))
}

/**
 * A claim the server refused for the doctor: Rust sends `error: detail`, so the message opens with
 * `doctor`. The detail (the failing steps, or "stale") is kun's own words and is shown as sent.
 */
export function isDoctorRefusal(message: string): boolean {
  return /^doctor\b/.test(message.replace(/^Error:\s*/, "").trim())
}

/** The detail after `doctor:`, or "". */
export function doctorDetail(message: string): string {
  const m = message.replace(/^Error:\s*/, "").trim().match(/^doctor\s*:\s*([\s\S]*)$/)
  return m ? m[1].trim() : ""
}

/**
 * The quiet line on a task card the doctor adds without blocking: orange counts the extras missing,
 * unknown says it is still checking. Null when green or red (red blocks and says so itself).
 */
export function doctorNote(
  r: { doctorExtras: number; doctorChecking: boolean },
  isAr: boolean,
): { text: string; link: boolean } | null {
  if (r.doctorChecking) return { text: isAr ? "جارٍ الفحص…" : "Checking…", link: false }
  if (r.doctorExtras > 0) {
    const n = r.doctorExtras
    return { text: isAr ? `الطبيب: تنقص ${n} إضافات` : `Doctor: ${n} extra${n === 1 ? "" : "s"} missing`, link: true }
  }
  return null
}
