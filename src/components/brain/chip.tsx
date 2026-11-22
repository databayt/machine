"use client"

import { ChevronDown } from "lucide-react"
import { useCallback, useEffect, useState } from "react"
import Link from "next/link"

import { cn } from "@/lib/utils"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import type { Locale } from "@/components/local/config"

import { copy as chat, tr, type Copy } from "../chat/config"
import { FAMILIES, type Family, modelName } from "../chat/model-pure"
import { useInMachine } from "../shell/content"
import { brainApply, brainFallbackState, brainStatus } from "./actions"
import { copy } from "./config"
import type { Brain, BrainView, RouteStatus } from "./types"

const NAMES: Record<Brain, Copy> = {
  claude: copy.chipClaude,
  nvidia: copy.nvidiaName,
  openrouter: copy.openrouterName,
  omniroute: copy.omnirouteName,
}

const ready = (r: RouteStatus) => r.route === "claude" || (r.installed && r.key_set)

/**
 * The model chip by the composer: always there in the app. It shows the model the next run uses: the
 * route's or formula's default ("Opus 5.5") or the free route ("NVIDIA free"). The popover lists Opus, Sonnet
 * and Haiku 5.5, then the free routes that are ready. A Claude pick is for the next run only (`onOverride`,
 * reset by the caller after the send); a free route is the persistent brain switch, as before.
 * `model` is that default model string (a family or a full id); Fable is never offered.
 */
export function ModelChip({
  lang,
  disabled,
  session,
  model,
  override,
  onOverride,
}: {
  lang: Locale
  disabled?: boolean
  /** The chat's session id: a session that fell back from Claude shows its free route with "(fallback)". */
  session?: string | null
  model: string | null
  override: Family | null
  onOverride: (f: Family | null) => void
}) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const [view, setView] = useState<BrainView | null>(null)
  const [open, setOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [fallback, setFallback] = useState<Brain | null>(null)

  const load = useCallback(() => {
    brainStatus()
      .then((v) => v && setView(v))
      .catch(() => null)
  }, [])

  useEffect(() => {
    if (inApp) load()
  }, [inApp, load])

  // After each run (`disabled` falls back to false) and on a new session: did it move to a free route?
  useEffect(() => {
    if (!inApp || !session || disabled) return setFallback(null)
    brainFallbackState(session)
      .then(setFallback)
      .catch(() => setFallback(null))
  }, [inApp, session, disabled])

  if (!inApp) return null
  const current: Brain = view?.current ?? "claude"
  const free = (view?.routes ?? []).filter((r) => r.route !== "claude" && ready(r))

  async function pickFamily(f: Family) {
    setError(null)
    try {
      if (current !== "claude") setView(await brainApply("claude"))
      onOverride(f)
      setOpen(false)
    } catch (e) {
      setError(String(e))
    }
  }
  async function pickFree(route: Brain) {
    setError(null)
    try {
      setView(await brainApply(route))
      onOverride(null)
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
            aria-label={t(chat.modelChipLabel)}
            className="text-muted-foreground hover:text-foreground hover:bg-muted flex items-center gap-1 rounded-full px-3 py-1 text-xs transition-colors disabled:opacity-50"
          >
            <bdi>{fallback ? `${t(NAMES[fallback])} (${t(chat.fallback)})` : current === "claude" ? modelName(override ?? model) : t(NAMES[current])}</bdi>
            <ChevronDown className="size-3" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent align="center" dir={lang === "ar" ? "rtl" : "ltr"} className="w-56 rounded-xl p-1">
          {FAMILIES.map((f) => (
            <button
              key={f}
              type="button"
              onClick={() => void pickFamily(f)}
              className={cn(item, "justify-between", current === "claude" && (override ?? "") === f && "font-semibold")}
            >
              <bdi>{modelName(f)}</bdi>
              {current === "claude" && override === f && <span className="text-muted-foreground text-xs">{t(chat.modelNextOnly)}</span>}
            </button>
          ))}
          {free.length > 0 && <p className="text-muted-foreground px-2 pt-2 pb-1 text-xs">{t(chat.modelFree)}</p>}
          {free.map((r) => (
            <button key={r.route} type="button" onClick={() => void pickFree(r.route)} className={cn(item, r.route === current && "font-semibold")}>
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
