// spotlight — what the menu's banner rotates: the few most relevant things to DO, then to KNOW.
// Pure: it ranks data the menu already loaded (tasks, health, unread count) and writes the sentence
// templates the banner sets (`**bold phrase**` + `{mark}`, see banner.tsx toPieces). No I/O here.
import type { Task } from "../tasks/types"
import type { HealthReport, TasksView } from "./data"

export const MAX_SPOTLIGHT = 4
/** To-do never crowds to-know out entirely: with something to know, to-do keeps at most this many. */
const MAX_TODO_WITH_KNOW = 3

export type SpotlightKind = "continue" | "start" | "clone" | "sprint" | "notes" | "health" | "lane" | "connect"

export interface SpotlightItem {
  id: string
  /** The flip: to do, then to know. */
  side: "do" | "know"
  kind: SpotlightKind
  /** The sentence, `**the opening phrase**` with the data's `{mark}` inside it. */
  template: string
  /** DATA, never translated: a task number, a repo, a sprint title, a count. */
  mark: string
  /** Locale-less path (the banner prefixes `/<lang>`); null = nowhere to go. */
  href: string | null
  /** The task a Start / Continue pill acts on. */
  taskId?: string
}

export interface SpotlightInput {
  lang: "en" | "ar"
  view: TasksView | null
  /** The upcoming issues in queue order (what the cards show). */
  tasks: Task[]
  login: string | undefined
  /** Unread CEO notes (the `network:unread` event); 0 when unknown. */
  unread: number
  health: HealthReport | null
  /** The selected department's quick actions — the banner's to-do when no task is waiting. */
  lanes?: { label: string; prompt: string; detail?: string }[]
  /** The department's name, for the "connect" line. */
  department?: string
}

/** The sentence is split on `**` and `{mark}`: a title must not be able to reopen either. */
const clean = (s: string) => s.replace(/[*{}]/g, "").replace(/\s+/g, " ").trim()

/**
 * Two lines of the banner's headline hold about this many characters at its width; every sentence
 * is fitted to it, so the fixed two-line box never clips a third line.
 */
export const SENTENCE_MAX = 80

/** A title cut at a word near `max` characters, so the banner's two lines are never clipped. */
const short = (s: string, max = 44) => {
  if (max < 8) return ""
  const c = clean(s)
  if (c.length <= max) return c
  const cut = c.slice(0, max)
  return `${cut.slice(0, Math.max(cut.lastIndexOf(" "), max - 12)).trim()}…`
}

const T = {
  continue: { en: "**Continue {mark}** — {title}; it's yours this sprint.", ar: "**تابع {mark}** — {title}؛ المهمة لك في هذه الدورة." },
  start: { en: "**Start {mark}** — {title}; it's next and ready to go.", ar: "**ابدأ {mark}** — {title}؛ هي التالية وجاهزة." },
  clone: { en: "**Clone {mark}** to start {title}; the task needs this repo here.", ar: "**انسخ {mark}** لتبدأ {title}؛ المهمة تحتاج المستودع هنا." },
  sprint: { en: "**{mark}** is the current sprint; everything here follows its plan.", ar: "**{mark}** هي الدورة الحالية؛ كل ما هنا يتبع خطتها." },
  notes: { en: "**{mark}** from the CEO are waiting; read them before today's work.", ar: "**{mark}** من المدير بانتظارك؛ اقرأها قبل عمل اليوم." },
  health: { en: "**{mark}**", ar: "**{mark}**" },
  lane: { en: "**{mark}** — {title}", ar: "**{mark}** — {title}" },
  connect: { en: "**Connect this machine** to see {mark}'s sprint and the tasks assigned to you.", ar: "**صِل هذا الجهاز** لترى دورة {mark} والمهام المسندة إليك." },
} as const

const noteCount = (n: number, ar: boolean) =>
  ar ? (n === 1 ? "ملاحظة واحدة" : n === 2 ? "ملاحظتان" : n <= 10 ? `${n} ملاحظات` : `${n} ملاحظة`) : n === 1 ? "1 note" : `${n} notes`

/** Fills `{title}` with as much of `text` as the 80-character sentence has room for. */
export function fit(template: string, mark: string, text: string): string {
  const rest = template.replace(/\*\*/g, "").replace("{mark}", mark).replace("{title}", "").length
  return template.replace("{title}", () => short(text, SENTENCE_MAX - rest))
}

export function rankSpotlight(input: SpotlightInput): SpotlightItem[] {
  const { lang, view, tasks, login, unread, health, lanes = [], department = "" } = input
  const ar = lang === "ar"
  const pick = (k: keyof typeof T) => T[k][lang]
  const num = (t: Task) => `#${t.number}`
  const mine = (t: Task) => !!login && t.holder === login
  const free = (t: Task) => !t.holder
  // Queue order, P0 first; a stable sort keeps the queue order inside one priority.
  const byPriority = (a: Task, b: Task) => (a.priority ?? 99) - (b.priority ?? 99)

  const todo: SpotlightItem[] = []
  const seen = new Set<string>()
  const add = (list: SpotlightItem[], it: SpotlightItem) => {
    if (seen.has(it.id)) return
    seen.add(it.id)
    list.push(it)
  }

  if (view) {
    for (const t of tasks.filter(mine))
      add(todo, {
        id: `continue:${t.id}`,
        side: "do",
        kind: "continue",
        template: fit(pick("continue"), num(t), t.title),
        mark: num(t),
        href: `/?task=${encodeURIComponent(t.id)}`,
        taskId: t.id,
      })

    const rest = tasks.filter((t) => !mine(t) && free(t)).sort(byPriority)
    for (const t of rest)
      if (view.readiness?.[t.id]?.state === "ready" || (!view.readiness?.[t.id] && !t.afterOpen.length))
        add(todo, {
          id: `start:${t.id}`,
          side: "do",
          kind: "start",
          template: fit(pick("start"), num(t), t.title),
          mark: num(t),
          href: `/?task=${encodeURIComponent(t.id)}`,
          taskId: t.id,
        })

    // Blocked only by a repo that is not on this computer: the way in is to clone it.
    for (const t of rest) {
      const r = view.readiness?.[t.id]
      if (!r || r.state !== "missing") continue
      const repo = r.missing.find((m) => m.startsWith("repo:"))?.slice(5)
      if (!repo) continue
      add(todo, {
        id: `clone:${repo}`,
        side: "do",
        kind: "clone",
        template: pick("clone").replace("{title}", () => num(t)),
        mark: repo,
        href: "/install/?stage=repos&step=all",
      })
    }
  }

  const know: SpotlightItem[] = []
  if (view?.sprint) {
    const { title, goal } = view.sprint
    add(know, {
      id: "sprint",
      side: "know",
      kind: "sprint",
      template: goal ? fit("**{mark}** — {title}", clean(title), goal) : pick("sprint"),
      mark: clean(title),
      href: null,
    })
  }
  if (unread > 0)
    add(know, { id: "notes", side: "know", kind: "notes", template: pick("notes"), mark: noteCount(unread, ar), href: null })
  for (const h of health?.items ?? [])
    if (!h.ok && !h.healed)
      add(know, {
        id: `health:${h.key}`,
        side: "know",
        kind: "health",
        template: pick("health"),
        mark: clean(h.detail || h.key),
        href: "/engine/",
      })

  // Never an empty banner: with no task waiting, the department's own lanes are today's to-do;
  // with no live sprint (the machine is not connected yet), say so as the thing to know.
  if (todo.length === 0)
    for (const l of lanes.slice(0, 2))
      add(todo, {
        id: `lane:${l.prompt}`,
        side: "do",
        kind: "lane",
        template: fit(pick("lane"), clean(l.label), l.detail || (ar ? "بداية جيدة لليوم؛ افتحها ويرشدك كن." : "a good place to start today; Kun walks you through it.")),
        mark: clean(l.label),
        href: `/?prompt=${encodeURIComponent(l.prompt)}`,
      })
  if (!view)
    add(know, { id: "connect", side: "know", kind: "connect", template: pick("connect"), mark: clean(department), href: "/settings/" })

  const todoTake = know.length ? Math.min(todo.length, MAX_TODO_WITH_KNOW) : MAX_SPOTLIGHT
  return [...todo.slice(0, todoTake), ...know].slice(0, MAX_SPOTLIGHT)
}
