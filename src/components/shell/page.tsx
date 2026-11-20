import { cn } from "@/lib/utils"

import { SettingsTabs } from "./tabs"

/**
 * The work area frame, the same as the mega menu's column 2: a `min-w-0` column of sections with
 * `gap-10`, a heading and a quiet description on top. The layout scrolls it; below `lg` it clears the
 * floating menu button, on `lg` it sits level with the sidebar.
 */
export function PageShell({
  title,
  description,
  actions,
  className,
  children,
}: {
  title: string
  description?: React.ReactNode
  /** Buttons at the heading's end. */
  actions?: React.ReactNode
  className?: string
  children: React.ReactNode
}) {
  return (
    <div className="min-w-0 ps-[26px] pe-6 pb-10 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:ps-0 lg:pe-16 lg:pb-16 lg:pt-16">
      <div className={cn("flex min-w-0 max-w-4xl flex-col gap-10", className)}>
        <SettingsTabs />
        <header className="flex min-w-0 flex-wrap items-start justify-between gap-x-6 gap-y-3">
          <div className="min-w-0">
            <h1 className="text-balance font-['thmanyah_serif_display',var(--font-sans)] text-3xl font-bold leading-snug tracking-tight lg:text-[38px]">
              {title}
            </h1>
            {description && <p className="mt-2 text-sm text-muted-foreground">{description}</p>}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
        </header>
        {children}
      </div>
    </div>
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
