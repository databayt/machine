"use client"

// readiness — small pieces the three install screens share.
import type { ReactNode } from "react"

import { statusMark, statusWord, tone } from "./config"
import type { StepStatus } from "./types"

export const button =
  "inline-flex h-9 items-center rounded-full bg-foreground px-4 text-sm font-medium text-background transition-colors hover:bg-foreground/90 disabled:opacity-50"
export const quiet =
  "inline-flex h-9 items-center rounded-full bg-muted px-4 text-sm font-medium transition-colors hover:bg-muted/70 disabled:opacity-50"

export function Mark({ status }: { status: StepStatus }) {
  return (
    <span className={`w-5 shrink-0 text-center font-semibold ${tone(status)}`} aria-hidden>
      {statusMark[status]}
    </span>
  )
}

export const word = (status: StepStatus, isAr: boolean) => (isAr ? statusWord[status].ar : statusWord[status].en)

/** One quiet card with a sentence, used for "nothing here" and the like. */
export function Quiet({ children }: { children: ReactNode }) {
  return <p className="rounded-2xl bg-background p-4 text-sm text-muted-foreground">{children}</p>
}

/** The error and notice lines every screen shows. */
export function Messages({ error, notice }: { error: string | null; notice: string | null }) {
  return (
    <>
      {error && <p className="whitespace-pre-wrap text-sm text-destructive">{error}</p>}
      {notice && <p className="text-sm text-muted-foreground">{notice}</p>}
    </>
  )
}

/** A mono pane that scrolls inside itself. */
export function LogPane({ lines }: { lines: string[] }) {
  if (lines.length === 0) return null
  return (
    <pre className="max-h-56 overflow-auto whitespace-pre-wrap rounded-2xl bg-background p-4 font-mono text-xs text-muted-foreground" dir="ltr">
      {lines.join("\n")}
    </pre>
  )
}
