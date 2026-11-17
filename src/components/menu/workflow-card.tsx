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
  /** Asks the agent how this workflow works (or, for a home card, its kun.databayt.org page). */
  howHref: string
  /** A kun.databayt.org home icon under `/icons/home/` — the site's own raster, used as-is. */
  image?: string | null
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
 * One workflow: a white tile with a large filled icon, the workflow's name and two lines on what it does. The
 * whole tile is one link to the composer starter.
 */
export function WorkflowCard({ w }: { w: Workflow; isAr: boolean }): React.JSX.Element {
  // The title and its starter, not the key: a quick action's key is "qa:…", which reads as QA.
  const Glyph = iconFor(`${w.title} ${w.description}`)
  const icon = w.image ? (
    // The site's own icon file, copied in public/icons/home; it inverts in dark mode as the site does.
    <img src={`/icons/home/${w.image}`} alt="" aria-hidden className="size-12 object-contain dark:invert" draggable={false} />
  ) : (
    <Glyph aria-hidden weight="fill" className="size-14 text-foreground/85 transition-colors group-hover:text-foreground" />
  )
  return (
    <Link
      href={w.href}
      className="group flex min-h-[240px] flex-col justify-between rounded-3xl bg-background p-7 text-start text-foreground outline-none transition-transform focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.99]"
    >
      {icon}
      <div>
        <h3 dir="auto" className="text-xl font-semibold leading-[1.15] tracking-tight lg:text-2xl">
          {w.title}
        </h3>
        <p dir="auto" className="mt-2 line-clamp-2 min-h-[2.5rem] text-sm leading-5 text-muted-foreground">
          {w.description}
        </p>
      </div>
    </Link>
  )
}
