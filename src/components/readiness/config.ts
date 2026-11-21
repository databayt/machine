// readiness — the words, both languages, and the small helpers the page and the agents tab share.
import type { FixKind, StepStatus } from "./types"

export type Text = { en: string; ar: string }

export const copy = {
  title: { en: "Readiness", ar: "الجاهزية" },
  subtitle: {
    en: "From joining the org to a fully configured computer. Every check and fix runs in kun; this page shows it.",
    ar: "من الانضمام إلى المنظمة حتى جهاز مهيّأ بالكامل. كل فحص وإصلاح يعمل في kun وهذه الصفحة تعرضه.",
  },
  check: { en: "Check now", ar: "افحص الآن" },
  complete: { en: "Complete setup", ar: "أكمل الإعداد" },
  update: { en: "Update everything", ar: "حدّث كل شيء" },
  heal: { en: "Heal with agent", ar: "عالج بالوكيل" },
  cancel: { en: "Cancel", ar: "إلغاء" },
  running: { en: "Running…", ar: "جارٍ التشغيل…" },
  noCatalog: {
    en: "kun has no readiness catalog on this computer yet. Update the kun checkout, then check again.",
    ar: "لا يوجد كتالوج جاهزية من kun على هذا الجهاز بعد. حدّث نسخة kun ثم افحص مجدداً.",
  },
  neverRun: { en: "No check has run yet.", ar: "لم يُجرَ أي فحص بعد." },
  done: { en: "ready", ar: "جاهز" },
  agentOut: { en: "Agent output", ar: "مخرجات الوكيل" },
  sent: { en: "Sent", ar: "أُرسل" },
} satisfies Record<string, Text>

export const fixLabel: Record<FixKind, Text> = {
  auto: { en: "Fix", ar: "أصلح" },
  terminal: { en: "Open Terminal", ar: "افتح الطرفية" },
  link: { en: "Open page", ar: "افتح الصفحة" },
  request: { en: "Ask for it", ar: "اطلبه" },
}

export const statusMark: Record<StepStatus, string> = {
  pending: "·",
  pass: "✓",
  fixed: "✓",
  updated: "✓",
  fail: "✗",
  blocked: "✗",
  skip: "–",
  fixing: "…",
  human: "☝",
  "update-available": "↑",
  removing: "…",
  removed: "–",
  unknown: "?",
}

export const statusWord: Record<StepStatus, Text> = {
  pending: { en: "not checked", ar: "لم يُفحص" },
  pass: { en: "ok", ar: "سليم" },
  fixed: { en: "fixed", ar: "أُصلح" },
  updated: { en: "updated", ar: "حُدّث" },
  fail: { en: "needs work", ar: "يحتاج عملاً" },
  blocked: { en: "waiting on another step", ar: "ينتظر خطوة أخرى" },
  skip: { en: "skipped", ar: "تم تخطيه" },
  fixing: { en: "fixing", ar: "قيد الإصلاح" },
  human: { en: "needs a person", ar: "يحتاج شخصاً" },
  "update-available": { en: "update available", ar: "تحديث متاح" },
  removing: { en: "removing", ar: "قيد الإزالة" },
  removed: { en: "removed", ar: "أُزيل" },
  unknown: { en: "unknown", ar: "غير معروف" },
}

export function tone(status: StepStatus): string {
  if (status === "pass" || status === "fixed" || status === "updated") return "text-emerald-600"
  if (status === "fail" || status === "blocked") return "text-destructive"
  if (status === "human" || status === "update-available") return "text-amber-600"
  return "text-muted-foreground"
}

/** A catalog string in the page's language, falling back to the other, then the id. */
export function pick(t: { ar: string; en: string } | undefined, isAr: boolean, fallback: string): string {
  if (!t) return fallback
  return (isAr ? t.ar || t.en : t.en || t.ar) || fallback
}
