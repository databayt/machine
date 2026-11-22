"use client"

import { useState } from "react"

import { copy, tr } from "./config"
import { foldBrief } from "./intake-pure"

/**
 * The finished brief, inside an assistant turn: its title, the prompt folded to two lines with a toggle,
 * and Start (runs it) and Edit (puts it in the box). The buttons show only while `canAct`.
 */
export function BriefTurn({
  title,
  prompt,
  isAr,
  canAct,
  onStart,
  onEdit,
}: {
  title: string
  prompt: string
  isAr: boolean
  canAct: boolean
  onStart: () => void
  onEdit: () => void
}) {
  const t = tr(isAr)
  const [open, setOpen] = useState(false)
  const fold = foldBrief(prompt)
  return (
    <div className="space-y-2">
      <p dir="auto" className="text-sm font-semibold">{title}</p>
      <p dir="auto" className="whitespace-pre-wrap break-words text-start text-[15px] leading-6 text-muted-foreground">
        {open ? prompt : fold.head}
        {!open && fold.hasMore && "…"}
      </p>
      {fold.hasMore && (
        <button type="button" onClick={() => setOpen(!open)} aria-expanded={open} className="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline">
          {open ? t(copy.briefHide) : t(copy.briefShow)}
        </button>
      )}
      {canAct && (
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <button
            type="button"
            onClick={onStart}
            className="inline-flex items-center rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background transition-colors hover:bg-foreground/85"
          >
            {t(copy.briefStart)}
          </button>
          <button
            type="button"
            onClick={onEdit}
            className="inline-flex items-center rounded-full bg-muted px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted/70"
          >
            {t(copy.briefEdit)}
          </button>
        </div>
      )}
    </div>
  )
}
