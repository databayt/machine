"use client"

import { Loader2, MoreHorizontal, Plus } from "lucide-react"
import { useState } from "react"

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { cn } from "@/lib/utils"

import { iconButton, type Item } from "../plugins/card"
import { pick } from "../readiness/config"
import type { StepView } from "../readiness/types"
import type { Readiness } from "../readiness/use-readiness"
import { copy } from "./config"
import { appMeta } from "./registry"

type T = (c: { en: string; ar: string }) => string

/** A step as a card's item: the registry's name, mark and line, else the catalog's title. */
export function itemOf(s: StepView, isAr: boolean): Item {
  const m = appMeta(s.id)
  return { id: s.id, name: m?.name ?? pick(s.title, isAr, s.id), logo: m?.logo, blurb: m?.blurb ?? { en: "", ar: "" } }
}

/** `+` installs one that is missing; `…` with Uninstall for one on the machine. */
export function AppAction({ s, r, installed, t, className }: { s: StepView; r: Readiness; installed: boolean; t: T; className?: string }) {
  const [open, setOpen] = useState(false)
  const name = appMeta(s.id)?.name ?? s.id
  if (r.busy)
    return (
      <span className={cn(iconButton, className, "opacity-100")} aria-busy>
        <Loader2 className="size-4 animate-spin" aria-hidden />
      </span>
    )
  if (!installed)
    return (
      <button type="button" aria-label={`${t(copy.install)} ${name}`} title={t(copy.install)} onClick={() => void r.act(s)} className={cn(iconButton, className)}>
        <Plus className="size-4" aria-hidden />
      </button>
    )
  if (!s.uninstall) return null
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" aria-label={`${t(copy.more)} ${name}`} className={cn(iconButton, className, open && "opacity-100")}>
          <MoreHorizontal className="size-4" aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-40 rounded-xl p-1">
        <button
          type="button"
          onClick={() => {
            setOpen(false)
            void r.uninstall(s.id)
          }}
          className="hover:bg-muted text-destructive flex w-full items-center rounded-md px-2 py-1.5 text-start text-sm transition-colors"
        >
          {t(copy.uninstall)}
        </button>
      </PopoverContent>
    </Popover>
  )
}

