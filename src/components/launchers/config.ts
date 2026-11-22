// launchers — every string of Settings → Agents, ar + en.
export const copy = {
  title: { en: "Agents", ar: "الوكلاء" },
  intro: {
    en: "The one-letter commands that start your agents in the terminal. Switch one off and its letter is removed from your shell.",
    ar: "أوامر الحرف الواحد التي تشغّل وكلاءك في الطرفية. أطفئ أحدها فيُزال حرفه من الصدفة.",
  },
  newTerminal: { en: "Open a new terminal for changes to apply.", ar: "افتح طرفية جديدة لتسري التغييرات." },
  bypass: { en: "Runs without permission prompts", ar: "يعمل دون أسئلة أذونات" },
  noBypass: { en: "Asks before it acts", ar: "يسأل قبل أن ينفّذ" },
  bypassConfig: { en: "Runs without prompts (set in opencode's config)", ar: "يعمل دون أسئلة (من إعدادات opencode)" },
  installed: { en: "Installed", ar: "مثبّت" },
  notInstalled: { en: "Not installed", ar: "غير مثبّت" },
  install: { en: "Install", ar: "ثبّت" },
  installing: { en: "Installing…", ar: "جارٍ التثبيت…" },
  on: { en: "On", ar: "مفعّل" },
  off: { en: "Off", ar: "متوقف" },
  files: { en: "Written to", ar: "يُكتب في" },
  noFile: {
    en: "kun has no launchers list on this computer yet. Update the kun checkout, then open this page again.",
    ar: "لا توجد قائمة أوامر من kun على هذا الجهاز بعد. حدّث نسخة kun ثم افتح الصفحة مجدداً.",
  },
  saved: { en: "Saved.", ar: "تم الحفظ." },
} as const
