// menu — every string and starter of the mega menu, ar + en.
import type { Copy } from "../chat/config"

export const copy = {
  open: { en: "Open menu", ar: "افتح القائمة" },
  close: { en: "Close menu", ar: "أغلق القائمة" },
  title: { en: "Menu", ar: "القائمة" },
  newChat: { en: "New chat", ar: "محادثة جديدة" },
  chats: { en: "Chats", ar: "المحادثات" },
  noChats: { en: "No chats yet. Ask something and it is kept here.", ar: "لا محادثات بعد. اسأل شيئاً وستُحفظ هنا." },
  settings: { en: "Settings", ar: "الإعدادات" },
  catalog: { en: "Catalog", ar: "الكتالوج" },
  catalogHint: { en: "Browse and install Kun, item by item", ar: "تصفّح كن وثبّته عنصراً عنصراً" },
  engine: { en: "Engine", ar: "المحرك" },
  engineHint: { en: "Version, updates and health", ar: "الإصدار والتحديثات والصحة" },
  start: { en: "Start", ar: "ابدأ" },
  repos: { en: "Repos", ar: "المستودعات" },
  recent: { en: "Recent chats", ar: "آخر المحادثات" },
  language: { en: "Language", ar: "اللغة" },
  health: { en: "Machine health", ar: "صحة الجهاز" },
  healthOk: { en: "Healthy", ar: "سليم" },
  healthBad: { en: "Needs attention", ar: "يحتاج انتباهاً" },
  healthUnknown: { en: "Not checked yet", ar: "لم يُفحص بعد" },
  tasksOffline: { en: "Tasks load when the machine is connected.", ar: "تظهر المهام حين يتصل الجهاز." },
  noSprintTasks: { en: "No sprint tasks for this department yet.", ar: "لا مهام في هذه الدورة بعد" },
  continue: { en: "Continue", ar: "تابع" },
  pillOpen: { en: "Open", ar: "افتح" },
  acknowledge: { en: "Acknowledge", ar: "اطلعت" },
  howItWorks: { en: "How it works", ar: "كيف تعمل؟" },
  workflows: { en: "Workflows", ar: "المسارات" },
  addRepo: { en: "Add repos", ar: "أضف المستودعات" },
  openIssue: { en: "Open on GitHub", ar: "افتح على GitHub" },
  todayHint: { en: "Suggested for today — tap to start", ar: "مقترحات اليوم — اضغط للبدء" },
  noDept: { en: "No department is set for this machine.", ar: "لا قسم لهذا الجهاز." },
  appOnly: { en: "Chats and lanes run inside the Kun app.", ar: "المحادثات والمسارات تعمل داخل تطبيق كن." },
  groups: {
    today: { en: "Today", ar: "اليوم" },
    yesterday: { en: "Yesterday", ar: "أمس" },
    week: { en: "Previous 7 days", ar: "آخر 7 أيام" },
    older: { en: "Older", ar: "أقدم" },
  },
} satisfies Record<string, Copy | Record<string, Copy>>

/** The repos a person can learn from, with the names shown for them. */
export const repoNames: Record<string, Copy> = {
  kun: { en: "Kun", ar: "كن" },
  hogwarts: { en: "Hogwarts", ar: "هوغورتس" },
  mkan: { en: "Mkan", ar: "مكان" },
}

export const defaultRepos = ["kun", "hogwarts", "mkan"]

export function repoName(repo: string, isAr: boolean): string {
  const n = repoNames[repo]
  return n ? (isAr ? n.ar : n.en) : repo
}

/** Asks the agent to walk through one skill. */
export const howStarter = (name: string, isAr: boolean) =>
  isAr ? `اشرح كيف تعمل مهارة ${name} خطوة بخطوة` : `Explain how the ${name} skill works, step by step`

/** Starter for one skill. */
export const skillStarter = (title: string, isAr: boolean) => (isAr ? `أريد استخدام مهارة ${title}: ` : `Use the ${title} skill to `)

export const promptHref = (lang: string, prompt: string) => `/${lang}/?prompt=${encodeURIComponent(prompt)}`

/** Window events that tie the menu to the open chat. */
export const NEW_CHAT_EVENT = "kun:new-chat"
export const CHAT_DELETED_EVENT = "kun:chat-deleted"

export const DEPT_KEY = "kun.machine.dept"
