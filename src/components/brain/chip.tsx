"use client"

import { ChevronDown } from "lucide-react"
import { useCallback, useEffect, useState } from "react"
import Link from "next/link"

import { cn } from "@/lib/utils"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import type { Locale } from "@/components/local/config"

import { tr, type Copy } from "../chat/config"
import { useInMachine } from "../shell/content"
import { brainApply, brainStatus } from "./actions"
import { copy } from "./config"
import type { Brain, BrainView, RouteStatus } from "./types"

const NAMES: Record<Brain, Copy> = {
  claude: copy.chipClaude,
  nvidia: copy.nvidiaName,
  openrouter: copy.openrouterName,
  omniroute: copy.omnirouteName,
}

const ready = (r: RouteStatus) => r.route === "claude" || (r.installed && r.key_set)

/** A quiet pill by the composer: the model a run uses now, and a switch among the routes that are ready.
 *  Hidden outside the app and while no free route is ready (Claude alone needs no choice). */
export function ModelChip({ lang, disabled }: { lang: Locale; disabled?: boolean }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const [view, setView] = useState<BrainView | null>(null)
  const [open, setOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(() => {
    brainStatus()
      .then((v) => v && setView(v))
      .catch(() => null)
  }, [])

  useEffect(() => {
    if (inApp) load()
  }, [inApp, load])

  if (!inApp || !view) return null
  const usable = view.routes.filter(ready)
  if (!usable.some((r) => r.route !== "claude")) return null

  async function pick(route: Brain) {
    setError(null)
    try {
      setView(await brainApply(route))
      setOpen(false)
    } catch (e) {
      setError(String(e))
    }
  }

  const item = "hover:bg-muted flex w-full items-center rounded-md px-2 py-1.5 text-start text-sm transition-colors"
  return (
    <div className="mt-2 flex justify-center">
      <Popover
        open={open}
        onOpenChange={(o) => {
          setOpen(o)
          if (o) load()
        }}
      >
        <PopoverTrigger asChild>
          <button
            type="button"
            disabled={disabled}
            aria-label={t(copy.chipLabel)}
            className="text-muted-foreground hover:text-foreground hover:bg-muted flex items-center gap-1 rounded-full px-3 py-1 text-xs transition-colors disabled:opacity-50"
          >
            <bdi>{t(NAMES[view.current])}</bdi>
            <ChevronDown className="size-3" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent align="center" dir={lang === "ar" ? "rtl" : "ltr"} className="w-56 rounded-xl p-1">
          {usable.map((r) => (
            <button key={r.route} type="button" onClick={() => void pick(r.route)} className={cn(item, r.route === view.current && "font-semibold")}>
              <bdi>{t(NAMES[r.route])}</bdi>
            </button>
          ))}
          <Link href={`/${lang}/model/`} onClick={() => setOpen(false)} className={cn(item, "text-muted-foreground")}>
            {t(copy.manage)}
          </Link>
          {error && <p className="text-destructive px-2 py-1 text-xs">{error}</p>}
        </PopoverContent>
      </Popover>
    </div>
  )
}
