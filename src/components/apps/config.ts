// apps (inside Plugins) — every string they show, ar + en.
import type { Copy } from "../chat/config.ts"

export const copy = {
  title: { en: "Apps", ar: "التطبيقات" },
  install: { en: "Install", ar: "ثبّت" },
  uninstall: { en: "Uninstall", ar: "أزل التثبيت" },
  more: { en: "More for", ar: "خيارات" },
  installed: { en: "Installed", ar: "مثبّت" },
  notInstalled: { en: "Not installed", ar: "غير مثبّت" },
  hint: { en: "installed", ar: "مثبّتة" },
} satisfies Record<string, Copy>
