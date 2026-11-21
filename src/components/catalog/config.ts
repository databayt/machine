// catalog — every string the control room shows, ar + en (the `{ en, ar }` + `tr(isAr)` pattern).
import type { EntryState } from "./types"

export interface Copy {
  en: string
  ar: string
}

export const tr = (isAr: boolean) => (k: Copy) => (isAr ? k.ar : k.en)

export const dirLabel: Record<string, Copy> = {
  skills: { en: "Skills", ar: "المهارات" },
  agents: { en: "Agents", ar: "الوكلاء" },
  rules: { en: "Rules", ar: "القواعد" },
  hooks: { en: "Hooks", ar: "الخطافات" },
  departments: { en: "Departments", ar: "الأقسام" },
  mcp: { en: "MCP", ar: "MCP" },
}

/** The singular, for the kind line: "Skill · from kun". */
export const kindLabel: Record<string, Copy> = {
  skills: { en: "Skill", ar: "مهارة" },
  agents: { en: "Agent", ar: "وكيل" },
  rules: { en: "Rule", ar: "قاعدة" },
  hooks: { en: "Hook", ar: "خطّاف" },
  departments: { en: "Department", ar: "قسم" },
  mcp: { en: "MCP server", ar: "خادم MCP" },
}

export const stateLabel: Record<EntryState, Copy> = {
  installed: { en: "Installed", ar: "مثبّت" },
  outdated: { en: "Update available", ar: "يوجد تحديث" },
  available: { en: "Not on this computer", ar: "غير موجود على هذا الجهاز" },
  disabled: { en: "Switched off", ar: "متوقف" },
  personal: { en: "Yours", ar: "ملكك" },
}

export const copy = {
  loading: { en: "Loading…", ar: "جارٍ التحميل…" },
  empty: { en: "Nothing here.", ar: "لا شيء هنا." },
  notFound: { en: "Nothing here.", ar: "لا شيء هنا." },
  getMachine: { en: "Get machine", ar: "حمّل machine" },
  install: { en: "Install", ar: "تثبيت" },
  update: { en: "Update", ar: "تحديث" },
  installPack: { en: "Install pack", ar: "تثبيت الحزمة" },
  uninstall: { en: "Uninstall", ar: "إزالة" },
  working: { en: "Working…", ar: "جارٍ العمل…" },
  confirm: { en: "Remove this from the computer?", ar: "إزالة هذا من الجهاز؟" },
  remove: { en: "Remove", ar: "أزل" },
  cancel: { en: "Cancel", ar: "إلغاء" },
  switchOn: { en: "On", ar: "مفعّل" },
  fromKun: { en: "from kun", ar: "من kun" },
  yours: { en: "yours", ar: "ملكك" },
  personalNote: {
    en: "This one is yours. The app never deletes it; you can only switch it off.",
    ar: "هذا العنصر ملكك. لا يحذفه التطبيق أبداً؛ يمكنك إيقافه فقط.",
  },
  mcpOutside: {
    en: "MCP servers are managed in the app.",
    ar: "تُدار خوادم MCP داخل التطبيق.",
  },
  mcpRestored: {
    en: "Health went RED, so the previous content was restored.",
    ar: "تحول الفحص إلى الأحمر فأُعيد المحتوى السابق.",
  },
  saved: { en: "Saved", ar: "تم الحفظ" },
  health: { en: "health", ar: "الفحص" },
  connected: { en: "Connected", ar: "متصل" },
  notConnected: { en: "Not connected", ar: "غير متصل" },
  unknown: { en: "Connection not known", ar: "حالة الاتصال غير معروفة" },
  skipped: { en: "Setup skips it", ar: "الإعداد يتخطاه" },
  files: { en: "Files", ar: "الملفات" },
  needs: { en: "Needs", ar: "يحتاج" },
  keywords: { en: "Keywords", ar: "الكلمات المفتاحية" },
  version: { en: "Version", ar: "الإصدار" },
  description: { en: "About", ar: "نبذة" },
  contains: { en: "Contains", ar: "يحتوي" },
} satisfies Record<string, Copy>

/** The quiet count line at the bottom of a folder: `86 items · 61 on`. */
export function countLine(isAr: boolean, total: number, on: number): string {
  return isAr ? `${total} عنصر · ${on} مفعّل` : `${total} ${total === 1 ? "item" : "items"} · ${on} on`
}

export function kindLine(isAr: boolean, dir: string, owner: "kun" | "you"): string {
  const t = tr(isAr)
  return `${t(kindLabel[dir] ?? kindLabel.skills)} · ${owner === "kun" ? t(copy.fromKun) : t(copy.yours)}`
}
