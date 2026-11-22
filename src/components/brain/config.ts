// brain — every string of the Model page and the composer chip, ar + en.
import type { Copy } from "../chat/config"

export const copy = {
  title: { en: "Model", ar: "النموذج" },
  intro: {
    en: "Which model runs your agent. Claude on your own sign-in is the default; a free route is optional and costs nothing.",
    ar: "أي نموذج يشغّل وكيلك. كلود بحسابك هو الافتراضي، والمسار المجاني اختياري ولا يكلّف شيئاً.",
  },
  use: { en: "Use this", ar: "استخدم هذا" },
  using: { en: "In use", ar: "قيد الاستخدام" },
  save: { en: "Save key", ar: "احفظ المفتاح" },
  keyPlaceholder: { en: "Paste your key", ar: "ألصق مفتاحك" },
  keyReplace: { en: "Replace key", ar: "استبدل المفتاح" },
  install: { en: "Install", ar: "ثبّت" },
  installing: { en: "Installing…", ar: "جارٍ التثبيت…" },
  installed: { en: "Installed", ar: "مثبّت" },
  notInstalled: { en: "Not installed", ar: "غير مثبّت" },
  keySet: { en: "Key saved", ar: "المفتاح محفوظ" },
  keyMissing: { en: "No key yet", ar: "لا مفتاح بعد" },
  proxyUp: { en: "Running", ar: "يعمل" },
  proxyDown: { en: "Starts with your next run", ar: "يبدأ مع تشغيلك التالي" },
  privacy: {
    en: "A free provider can read what a run sends; keep client code on Claude.",
    ar: "المزوّد المجاني يستطيع قراءة ما يرسله التشغيل؛ أبقِ شيفرة العملاء على كلود.",
  },
  claudeName: { en: "Claude", ar: "كلود" },
  claudeBlurb: { en: "Your own subscription", ar: "اشتراكك الخاص" },
  nvidiaName: { en: "NVIDIA free", ar: "NVIDIA مجاني" },
  nvidiaBlurb: { en: "Nemotron 3 Ultra 550B, about 40 requests a minute", ar: "Nemotron 3 Ultra 550B، نحو 40 طلباً في الدقيقة" },
  openrouterName: { en: "OpenRouter free", ar: "OpenRouter مجاني" },
  openrouterBlurb: {
    en: "50 requests a day, or 1,000 a day after a one-time $10 credit you buy yourself",
    ar: "50 طلباً في اليوم، أو 1,000 يومياً بعد رصيد 10 دولارات تشتريه بنفسك مرة واحدة",
  },
  omnirouteName: { en: "OmniRoute", ar: "OmniRoute" },
  omnirouteBlurb: { en: "Stacks every free tier into one route", ar: "يجمع كل الطبقات المجانية في مسار واحد" },
  getNvidiaKey: { en: "Get an NVIDIA key", ar: "احصل على مفتاح NVIDIA" },
  getOpenrouterKey: { en: "Get an OpenRouter key", ar: "احصل على مفتاح OpenRouter" },
  openrouterCredits: { en: "Add the $10 credit", ar: "أضف رصيد 10 دولارات" },
  omnirouteDashboard: {
    en: "Open the dashboard to add Google AI Studio, Groq and Mistral keys",
    ar: "افتح لوحة التحكم لإضافة مفاتيح Google AI Studio وGroq وMistral",
  },
  chipLabel: { en: "Model", ar: "النموذج" },
  chipClaude: { en: "Claude", ar: "كلود" },
  manage: { en: "Model settings", ar: "إعدادات النموذج" },
} satisfies Record<string, Copy>
