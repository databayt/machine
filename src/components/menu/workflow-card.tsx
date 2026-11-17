import Link from "next/link"
import {
  BarChart3Icon, BookOpenIcon, BriefcaseIcon, CalendarIcon, ClapperboardIcon, CodeIcon, FileTextIcon,
  MailIcon, MegaphoneIcon, MessageSquareIcon, PenLineIcon, SearchIcon, ShieldCheckIcon, SparklesIcon,
  TargetIcon, UsersIcon, VideoIcon, type LucideIcon,
} from "lucide-react"

import { copy } from "./config"
import { tr } from "../chat/config"

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

// A large icon per workflow, picked by keywords in its name; Sparkles when nothing matches.
const ICONS: [RegExp, LucideIcon][] = [
  [/video|film|edit|cut|reel|record|مونتاج|فيديو/i, VideoIcon],
  [/clip|shoot|motion|story/i, ClapperboardIcon],
  [/social|post|publish|carousel|campaign|brand|\bads?\b|منشور|حملة/i, MegaphoneIcon],
  [/lead|prospect|scrape|sales|outreach|funnel|client|عميل|مبيعات/i, TargetIcon],
  [/mail|email|inbox|newsletter|بريد/i, MailIcon],
  [/chat|reply|message|support|whatsapp|رسالة/i, MessageSquareIcon],
  [/report|metric|measure|analytic|stat|dashboard|تقرير|أرقام/i, BarChart3Icon],
  [/calendar|plan|schedule|weekly|week|خطة|جدول/i, CalendarIcon],
  [/write|draft|copy|caption|blog|اكتب|صياغة/i, PenLineIcon],
  [/doc|spec|contract|proposal|agreement|عقد|مستند/i, FileTextIcon],
  [/search|research|find|seek|jobs?\b|بحث/i, SearchIcon],
  [/code|build|deploy|fix|feature|ship|api|bug/i, CodeIcon],
  [/guard|security|audit|check|review|qa/i, ShieldCheckIcon],
  [/team|hire|people|onboard|فريق/i, UsersIcon],
  [/pricing|finance|invoice|budget|revenue|cost/i, BriefcaseIcon],
  [/learn|textbook|course|catalog|book|كتاب/i, BookOpenIcon],
]

export const iconFor = (name: string): LucideIcon => ICONS.find(([re]) => re.test(name))?.[1] ?? SparklesIcon

/**
 * One workflow, as the marketing site's product card: category line, big title, three lines of
 * description, and a large centred visual. The whole card is one link (the title's, stretched);
 * "How it works" sits above it, so there are never two anchors nested.
 */
export function WorkflowCard({ w, isAr }: { w: Workflow; isAr: boolean }): React.JSX.Element {
  const t = tr(isAr)
  const Icon = iconFor(`${w.key} ${w.title}`)
  return (
    <div className="group relative flex min-h-[460px] flex-col overflow-hidden rounded-3xl bg-muted p-8 text-start text-foreground md:p-10">
      <div>
        <p className="mb-1 text-sm font-medium tracking-tight text-foreground/80">{w.category}</p>
        <h3 dir="auto" className="text-2xl font-semibold leading-[1.1] tracking-tight md:text-3xl">
          <Link href={w.href} className="after:absolute after:inset-0 focus-visible:outline-none">
            {w.title}
          </Link>
        </h3>
        <p dir="auto" className="mt-3 line-clamp-3 min-h-[4.125rem] max-w-sm leading-snug text-foreground/70">
          {w.description}
        </p>
      </div>

      <div aria-hidden className="pointer-events-none flex flex-1 items-center justify-center py-6">
        <Icon className="h-24 w-auto text-foreground/25 transition-colors group-hover:text-foreground/40" strokeWidth={1} />
      </div>

      <Link
        href={w.howHref}
        className="relative z-10 w-fit text-sm text-muted-foreground underline underline-offset-4 transition-colors hover:text-foreground"
      >
        {t(copy.howItWorks)}
      </Link>
    </div>
  )
}
