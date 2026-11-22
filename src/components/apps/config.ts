// apps page — every string the page shows, ar + en.
import type { Copy } from "../chat/config.ts"

export const copy = {
  title: { en: "Apps", ar: "التطبيقات" },
  search: { en: "Search apps", ar: "ابحث في التطبيقات" },
  install: { en: "Install", ar: "ثبّت" },
  uninstall: { en: "Uninstall", ar: "أزل التثبيت" },
  more: { en: "More for", ar: "خيارات" },
  empty: { en: "Nothing matches.", ar: "لا نتائج مطابقة." },
  notFound: { en: "Not found.", ar: "غير موجود." },
  installed: { en: "Installed", ar: "مثبّت" },
  notInstalled: { en: "Not installed", ar: "غير مثبّت" },
  noCatalog: { en: "No readiness catalog on this computer yet.", ar: "لا يوجد كتالوج جاهزية على هذا الجهاز بعد." },
  hint: { en: "installed", ar: "مثبّتة" },
} satisfies Record<string, Copy>
