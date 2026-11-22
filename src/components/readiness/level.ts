// readiness — how the doctor's level reads: words in both languages and one quiet dot colour per
// level. Pure, so node can test it; the Setup card, the sidebar and the tools row all use it.
import type { GateLevel, GateView } from "./types.ts"

type Text = { en: string; ar: string }

const levelCopy = {
  green: { en: "Ready to contribute", ar: "جاهز للمساهمة" },
  orangeOne: { en: "Can contribute — 1 extra missing", ar: "يمكنك المساهمة — ينقص إضافي واحد" },
  orange: { en: "Can contribute — {n} extras missing", ar: "يمكنك المساهمة — تنقص {n} إضافات" },
  redOne: { en: "Contribution blocked — 1 core step missing", ar: "المساهمة متوقفة — تنقص خطوة أساسية واحدة" },
  red: { en: "Contribution blocked — {n} core steps missing", ar: "المساهمة متوقفة — تنقص {n} خطوات أساسية" },
  unknown: { en: "Checking…", ar: "جارٍ الفحص…" },
} satisfies Record<string, Text>

/** The number each level counts: core steps for red, extras for orange. */
export function levelCount(gate: Pick<GateView, "level" | "coreFailing" | "extraFailing">): number {
  return gate.level === "red" ? gate.coreFailing.length : gate.level === "orange" ? gate.extraFailing.length : 0
}

/** "Ready to contribute", "Can contribute — N extras missing", … in the language asked for. */
export function levelWords(gate: Pick<GateView, "level" | "coreFailing" | "extraFailing">, isAr: boolean): string {
  const n = levelCount(gate)
  const key: keyof typeof levelCopy =
    gate.level === "green" ? "green" : gate.level === "unknown" ? "unknown" : gate.level === "red" ? (n === 1 ? "redOne" : "red") : n === 1 ? "orangeOne" : "orange"
  const text = levelCopy[key]
  return (isAr ? text.ar : text.en).replace("{n}", String(n))
}

/** Tailwind background of the small dot: the quiet palette the machines page already uses. */
export const levelDot: Record<GateLevel, string> = {
  green: "bg-emerald-500",
  orange: "bg-amber-500",
  red: "bg-destructive",
  unknown: "border border-muted-foreground",
}

/** Tailwind text colour of the level's headline. */
export const levelText: Record<GateLevel, string> = {
  green: "text-emerald-600 dark:text-emerald-400",
  orange: "text-amber-600 dark:text-amber-400",
  red: "text-destructive",
  unknown: "text-muted-foreground",
}

/** The sidebar and the tools row show a dot only when something is missing (sidebar) or always (tools). */
export const needsAttention = (level: GateLevel): boolean => level === "orange" || level === "red"
