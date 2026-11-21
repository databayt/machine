import { Suspense } from "react"

import { cn } from "@/lib/utils"

import { BackButton } from "./back"
import { SHELL_TOP } from "./grid"

/**
 * The work area: bare. No title, no description, no toolbar, no tab row: the sidebar is the breadcrumb
 * and the content is the whole screen. `title` is only the screen's accessible name. A full-height
 * column whose content region is the one scroller (the frame around it does not scroll); below `lg`
 * it clears the floating menu button, on `lg` it sits level with the sidebar.
 */
export function PageShell({
  title,
  className,
  children,
}: {
  /** The screen's accessible name; never drawn. */
  title: string
  className?: string
  children: React.ReactNode
}) {
  return (
    <section aria-label={title} className="flex h-full min-h-0 min-w-0 flex-col">
      <div
        className={cn(
          "min-h-0 flex-1 overflow-y-auto overscroll-contain ps-[26px] pe-6 pb-10 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:ps-0 lg:pe-16 lg:pb-16",
          "flex flex-col",
          SHELL_TOP,
        )}
      >
        <Suspense fallback={<div className="mb-4 size-10" aria-hidden />}>
          <BackButton />
        </Suspense>
        <div className={cn("flex min-w-0 max-w-4xl flex-col gap-10", className)}>{children}</div>
      </div>
    </section>
  )
}

/** A quiet label over a body: the menu's section pattern. */
export function Section({
  title,
  action,
  className,
  children,
}: {
  title?: string
  action?: React.ReactNode
  className?: string
  children: React.ReactNode
}) {
  return (
    <section className={cn("flex min-w-0 flex-col gap-3", className)}>
      {(title || action) && (
        <div className="flex items-center justify-between gap-3">
          {title && <h2 className="text-sm text-muted-foreground">{title}</h2>}
          {action}
        </div>
      )}
      {children}
    </section>
  )
}
