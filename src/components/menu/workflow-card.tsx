import Link from "next/link"
import {
  BookOpen,
  Briefcase,
  CalendarBlank,
  ChartBar,
  ChatCircle,
  Code,
  Database,
  Envelope,
  FileText,
  FilmSlate,
  Funnel,
  MagnifyingGlass,
  Megaphone,
  PencilSimple,
  ShieldCheck,
  Sparkle,
  Target,
  UsersThree,
  VideoCamera,
  type Icon,
} from "@phosphor-icons/react"

export interface Workflow {
  key: string
  /** The department's title. */
  category: string
  title: string
  description: string
  /** The composer starter (`/<lang>/?prompt=…`). */
  href: string
  /** Asks the agent how this workflow works. */
  howHref: string
}

// A large filled icon per workflow, picked by keywords in its name; Sparkle when nothing matches.
// Specific steps run before broad rules, so a department's cards don't all wear one icon.
const ICONS: [RegExp, Icon][] = [
  [/video|film|edit|cut|reel|record|مونتاج|فيديو/i, VideoCamera],
  [/clip|shoot|motion|story/i, FilmSlate],
  [/social|post|publish|carousel|campaign|brand|\bads?\b|منشور|حملة/i, Megaphone],
  [/funnel|qualif|nurture|stalled|قمع|تأهيل/i, Funnel],
  [/scrap|enrich|crm|contact gap|اسحب/i, Database],
  [/find|search|prospect|seek|ابحث/i, MagnifyingGlass],
  [/lead|sales|outreach|client|عميل|مبيعات/i, Target],
  [/mail|email|inbox|newsletter|بريد/i, Envelope],
  [/chat|reply|message|support|whatsapp|follow|رسالة|متابعة|أتابع/i, ChatCircle],
  [/report|metric|measure|analytic|stat|dashboard|تقرير|أرقام/i, ChartBar],
  [/calendar|plan|schedule|weekly|week|خطة|جدول/i, CalendarBlank],
  [/write|draft|copy|caption|blog|اكتب|صياغة/i, PencilSimple],
  [/doc|spec|contract|proposal|offer|quote|agreement|عرض|عقد|مستند/i, FileText],
  [/code|build|deploy|fix|feature|ship|api|bug/i, Code],
  [/guard|security|audit|check|review|qa/i, ShieldCheck],
  [/team|hire|people|onboard|فريق/i, UsersThree],
  [/pricing|price|finance|invoice|budget|revenue|cost|سعر/i, Briefcase],
  [/learn|textbook|course|catalog|book|كتاب/i, BookOpen],
]

export const iconFor = (name: string): Icon => ICONS.find(([re]) => re.test(name))?.[1] ?? Sparkle

/**
 * One workflow: a white tile with a large filled icon and the workflow's name — nothing else. The
 * whole tile is one link to the composer starter.
 */
export function WorkflowCard({ w }: { w: Workflow; isAr: boolean }): React.JSX.Element {
  // The title and its starter, not the key: a quick action's key is "qa:…", which reads as QA.
  const Glyph = iconFor(`${w.title} ${w.description}`)
  return (
    <Link
      href={w.href}
      title={w.description}
      className="group flex min-h-[300px] flex-col justify-between rounded-3xl bg-background p-8 text-start text-foreground outline-none transition-transform focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.99] md:p-10"
    >
      <Glyph aria-hidden weight="fill" className="size-16 text-foreground/85 transition-colors group-hover:text-foreground" />
      <h3 dir="auto" className="text-2xl font-semibold leading-[1.1] tracking-tight md:text-3xl">
        {w.title}
      </h3>
    </Link>
  )
}
