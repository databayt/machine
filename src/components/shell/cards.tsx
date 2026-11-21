import Link from "next/link"

import { cn } from "@/lib/utils"

/** A grid of `NavCard`s on the `bg-muted` page: 2 columns from `sm`, 3 from `xl`. */
export function CardGrid({ className, children }: { className?: string; children: React.ReactNode }) {
  return <ul className={cn("grid gap-3 sm:grid-cols-2 xl:grid-cols-3", className)}>{children}</ul>
}

/**
 * One card, one link: the whole surface is the target. A name, an optional one-line hint under it and
 * an optional state (short text or a count) on the end side. No chevron, no underline.
 */
export function NavCard({
  href,
  name,
  hint,
  state,
  ltr,
}: {
  href: string
  name: React.ReactNode
  hint?: React.ReactNode
  state?: React.ReactNode
  /** The name is literal text (a folder, a file) and reads left to right in any language. */
  ltr?: boolean
}) {
  return (
    <li className="min-w-0">
      <Link
        href={href}
        className="flex h-full min-w-0 items-start justify-between gap-3 rounded-2xl bg-background p-4 no-underline outline-none transition-colors hover:bg-background/60 focus-visible:ring-2 focus-visible:ring-foreground/40"
      >
        <span className="min-w-0">
          <span dir={ltr ? "ltr" : undefined} className="block truncate font-medium">
            {name}
          </span>
          {hint && <span className="mt-1 block truncate text-sm text-muted-foreground">{hint}</span>}
        </span>
        {state && (
          <span className="shrink-0 text-sm text-muted-foreground tabular-nums" dir="ltr">
            {state}
          </span>
        )}
      </Link>
    </li>
  )
}
