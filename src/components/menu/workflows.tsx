"use client"

// The department's workflows as a carousel of cards — the marketing site's products carousel
// (marketing/src/components/marketing/products-carousel.tsx), adapted: its full-bleed clipper
// bleeds to the work area's edges, not the screen's.
import * as React from "react"
import useEmblaCarousel, { type UseEmblaCarouselType } from "embla-carousel-react"
import { ChevronLeft, ChevronRight } from "lucide-react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"
import type { Department } from "../profile/types"
import { tr } from "../chat/config"
import { copy, howStarter, promptHref, skillStarter } from "./config"
import { WorkflowCard, type Workflow } from "./workflow-card"

type EmblaApi = UseEmblaCarouselType[1]

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
    add({ key: `qa:${q.prompt}`, category, title, description: q.prompt, href: promptHref(lang, q.prompt), howHref: promptHref(lang, howStarter(title, isAr)) })
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
  const isRTL = lang === "ar"
  const t = tr(isRTL)
  const [emblaRef, emblaApi] = useEmblaCarousel({
    align: "start",
    direction: isRTL ? "rtl" : "ltr",
    loop: false,
    containScroll: "trimSnaps",
  })
  const [canPrev, setCanPrev] = React.useState(false)
  const [canNext, setCanNext] = React.useState(false)

  const onSelect = React.useCallback((api: EmblaApi) => {
    if (!api) return
    setCanPrev(api.canScrollPrev())
    setCanNext(api.canScrollNext())
  }, [])

  React.useEffect(() => {
    if (!emblaApi) return
    onSelect(emblaApi)
    emblaApi.on("select", onSelect).on("reInit", onSelect)
    return () => {
      emblaApi.off("select", onSelect).off("reInit", onSelect)
    }
  }, [emblaApi, onSelect])

  // The slide list changes with the department.
  React.useEffect(() => {
    emblaApi?.reInit()
  }, [emblaApi, workflows])

  if (workflows.length === 0) return null

  // In RTL the visual "previous" arrow points right, "next" points left.
  const PrevIcon = isRTL ? ChevronRight : ChevronLeft
  const NextIcon = isRTL ? ChevronLeft : ChevronRight

  return (
    // `@container` makes 100cqi resolve to the work area's width; card widths come from it.
    <section aria-label={t(copy.workflows)} className="@container min-w-0">
      {/* The clipper: exactly the work area, so a peeking card is cut at its edge (overflow-x-clip
          clips without creating scrollable overflow). */}
      <div className="overflow-x-clip">
        {/* Embla's viewport, re-contained to the container width and left overflow-visible, so one
            "next" advances exactly one card and the scroll rail equals the work area. */}
        <div className="mx-auto w-[100cqi] overflow-visible" ref={emblaRef}>
          <div className="flex gap-4">
            {workflows.map((w) => (
              <div
                key={w.key}
                className="min-w-0 shrink-0 grow-0 basis-[calc(100cqi/1.15)] md:basis-[calc((100cqi-1rem)/2)] lg:basis-[calc((100cqi-2rem)/3)]"
              >
                <WorkflowCard w={w} isAr={isRTL} />
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Apple-style paddles, at the end of the work area */}
      <div className="mt-6 flex justify-end gap-3">
        <PaddleButton onClick={() => emblaApi?.scrollPrev()} disabled={!canPrev} label={t(copy.prev)}>
          <PrevIcon className="size-5" strokeWidth={2.25} />
        </PaddleButton>
        <PaddleButton onClick={() => emblaApi?.scrollNext()} disabled={!canNext} label={t(copy.next)}>
          <NextIcon className="size-5" strokeWidth={2.25} />
        </PaddleButton>
      </div>
    </section>
  )
}

function PaddleButton({
  onClick,
  disabled,
  label,
  children,
}: {
  onClick: () => void
  disabled?: boolean
  label: string
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={cn(
        "flex size-9 items-center justify-center rounded-full bg-background text-foreground/70",
        "transition-colors hover:text-foreground",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
        "disabled:pointer-events-none disabled:opacity-40",
      )}
    >
      {children}
    </button>
  )
}
