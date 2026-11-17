"use client"

// The department's workflows as a carousel of cards — the marketing site's products carousel
// (marketing/src/components/marketing/products-carousel.tsx), adapted: its full-bleed clipper
// bleeds to the work area's edges, not the screen's.
import * as React from "react"

import type { Locale } from "@/components/local/config"
import type { Department } from "../profile/types"
import { tr } from "../chat/config"
import { copy, howStarter, promptHref, skillStarter } from "./config"
import { WorkflowCard, type Workflow } from "./workflow-card"


/** The department's quick actions, then its skills — deduped by name. */
export function workflowsFor({
  lang,
  dept,
  hidden,
}: {
  lang: Locale
  dept: Department | undefined
  hidden: string[]
}): Workflow[] {
  if (!dept) return []
  const isAr = lang === "ar"
  const category = (isAr ? dept.title_ar : null) ?? dept.title
  const seen = new Set<string>()
  const out: Workflow[] = []
  const add = (w: Workflow) => {
    const k = w.title.trim().toLowerCase()
    if (!k || seen.has(k)) return
    seen.add(k)
    out.push(w)
  }
  for (const q of dept.quick_actions) {
    const title = isAr ? q.label_ar : q.label_en
    add({ key: `qa:${q.prompt}`, category, title, description: (isAr ? q.description_ar : q.description_en) || title, href: promptHref(lang, q.prompt), howHref: promptHref(lang, howStarter(title, isAr)) })
  }
  for (const s of dept.skills) {
    if (hidden.includes(`skill:${s.name}`)) continue
    add({
      key: `skill:${s.name}`,
      category,
      title: s.title,
      description: s.description,
      href: promptHref(lang, skillStarter(s.title, isAr)),
      howHref: promptHref(lang, howStarter(s.name, isAr)),
    })
  }
  return out
}

export function Workflows({ lang, workflows }: { lang: Locale; workflows: Workflow[] }): React.JSX.Element | null {
  const t = tr(lang === "ar")
  if (workflows.length === 0) return null

  return (
    // `@container` makes 100cqi resolve to the work area's width; card widths come from it.
    // A native scroll row, not a carousel: trackpad, wheel and swipe scroll it right away, and the
    // browser handles the direction in Arabic. Four cards per view on large screens, a peek on phones.
    <section aria-label={t(copy.workflows)} className="@container min-w-0">
      <ul className="flex snap-x snap-mandatory gap-4 overflow-x-auto overscroll-x-contain scroll-smooth [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {workflows.map((w) => (
          <li
            key={w.key}
            className="min-w-0 shrink-0 grow-0 snap-start basis-[calc(100cqi/1.15)] md:basis-[calc((100cqi-1rem)/2.2)] lg:basis-[calc((100cqi-3rem)/4)]"
          >
            <WorkflowCard w={w} isAr={lang === "ar"} />
          </li>
        ))}
      </ul>
    </section>
  )
}
