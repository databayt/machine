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
  auto: { en: "Install", ar: "ثبّت" },
  terminal: { en: "Sign in", ar: "سجّل الدخول" },
  link: { en: "Open", ar: "افتح" },
  request: { en: "Ask Abdout", ar: "اطلبه" },
}

/** The install and uninstall wizard's words. */
export const wizard = {
  installAll: { en: "Install everything", ar: "ثبّت كل شيء" },
  check: { en: "Check", ar: "افحص" },
  update: { en: "Update", ar: "حدّث" },
  heal: { en: "Heal with agent", ar: "عالج بالوكيل" },
  cancel: { en: "Cancel", ar: "إلغاء" },
  install: { en: "Install", ar: "ثبّت" },
  installStage: { en: "Install this stage", ar: "ثبّت هذه المرحلة" },
  uninstall: { en: "Uninstall", ar: "أزل" },
  remove: { en: "Remove", ar: "أزل" },
  ready: { en: "This computer is ready", ar: "هذا الجهاز جاهز" },
  neverRun: { en: "No check has run yet", ar: "لم يُجرَ أي فحص بعد" },
  needWork: { en: "{n} steps still need work", ar: "{n} خطوات ما زالت تحتاج عملاً" },
  needWorkOne: { en: "1 step still needs work", ar: "خطوة واحدة ما زالت تحتاج عملاً" },
  hintDone: { en: "Done", ar: "تم" },
  hintNotChecked: { en: "Not checked yet", ar: "لم يُفحص بعد" },
  hintNeedYou: { en: "{n} need you", ar: "{n} تحتاجك" },
  hintToInstall: { en: "{n} to install", ar: "{n} للتثبيت" },
  agentOut: { en: "Agent output", ar: "مخرجات الوكيل" },
  nothing: { en: "Nothing here", ar: "لا شيء هنا" },
  removedByYou: { en: "Removed by you", ar: "أزلتَه أنت" },
  runs: { en: "What it runs", ar: "ما الذي يشغّله" },
  waitsFor: { en: "Waits for", ar: "ينتظر" },
  neededBy: { en: "Needed by", ar: "يحتاجه" },
  keptUpToDate: { en: "Kept up to date", ar: "يُبقى محدّثاً" },
  appliesTo: { en: "Applies to", ar: "ينطبق على" },
  auto: { en: "automatically", ar: "تلقائياً" },
  confirmRemove: { en: "This will remove", ar: "سيُزال" },
  dependents: { en: "These installed steps need it", ar: "هذه الخطوات المثبّتة تحتاجه" },
  keystone: {
    en: "Machine itself stands on this. After removing it the app cannot install anything. The one-line installer brings it back:",
    ar: "تطبيق Machine نفسه يقوم على هذا. بعد إزالته لن يستطيع التطبيق تثبيت أي شيء. المثبّت بسطر واحد يعيده:",
  },
  allRepos: { en: "All repositories", ar: "كل المستودعات" },
  defaultAgent: { en: "Default agent", ar: "الوكيل الافتراضي" },
  removedOk: { en: "Removed.", ar: "تمت الإزالة." },
  terminalOpen: {
    en: "Terminal is open. The removal finishes there.",
    ar: "الطرفية مفتوحة. تنتهي الإزالة هناك.",
  },
  cancelled: { en: "Cancelled.", ar: "أُلغي." },
  roles: { en: "Roles", ar: "الأدوار" },
  departments: { en: "Departments", ar: "الأقسام" },
  os: { en: "Systems", ar: "الأنظمة" },
} satisfies Record<string, Text>

export const KEYSTONE_INSTALLER = "curl -fsSL https://kun.databayt.org/install | bash"

export const fill = (t: Text, isAr: boolean, n: number): string => (isAr ? t.ar : t.en).replace("{n}", String(n))

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
