"use client"

import { useEffect, useRef, useState } from "react"
import { ChevronRightIcon, ListIcon, WrenchIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import { GLYPH_FRAME_MS, SHIMMER_CYCLE_MS, SHIMMER_STEP_MS, glyphFrame, statusDetail, summarize, turnWord } from "./activity-pure"
import { copy, tr, words } from "./config"

/** The agent's thinking mark: three dots that tumble (globals.css `.machine-dots`), in the line's colour. */
export function Dots({ className }: { className?: string }) {
  return (
    <span aria-hidden className={cn("machine-dots", className)}>
      <i />
      <i />
      <i />
    </span>
  )
}

/** True while the person asked the system for less motion. */
function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false)
  useEffect(() => {
    const q = window.matchMedia("(prefers-reduced-motion: reduce)")
    setReduced(q.matches)
    const on = () => setReduced(q.matches)
    q.addEventListener("change", on)
    return () => q.removeEventListener("change", on)
  }, [])
  return reduced
}

/** Claude Code's spinner glyph: · ✢ ✳ ✶ ✻ ✽ and back, one frame per `GLYPH_FRAME_MS`, a still ✻ under reduced motion. */
export function ClaudeGlyph({ className }: { className?: string }) {
  const reduced = useReducedMotion()
  const [frame, setFrame] = useState(4)
  useEffect(() => {
    if (reduced) return
    const id = window.setInterval(() => setFrame((n) => n + 1), GLYPH_FRAME_MS)
    return () => window.clearInterval(id)
  }, [reduced])
  return (
    <span aria-hidden dir="ltr" className={cn("inline-block w-[1.1em] shrink-0 text-center text-claude", className)}>
      {reduced ? "✻" : glyphFrame(frame)}
    </span>
  )
}

const row = "group flex max-w-full items-center gap-3 text-start text-[17px] leading-7 text-muted-foreground transition-colors hover:text-foreground md:text-base"
const icon = "size-[21px] shrink-0"

/**
 * The tools of a turn, the Claude iOS way: finished steps fold into one grey row ("Used Read, Bash +2")
 * that opens the list; the step running now is a live row, its name shimmering.
 */
export function ActivityRows({ done, live, isAr }: { done: string[]; live: string | null; isAr: boolean }) {
  const t = tr(isAr)
  const [open, setOpen] = useState(false)
  if (done.length === 0 && !live) return null

  const { head, rest } = summarize(done)
  const used = isAr ? "استخدم" : "Used"
  const headline = `${used} ${head.join(isAr ? "، " : ", ")}${rest ? ` +${rest}` : ""}`
  const toggle = open ? t(copy.hideSteps) : t(copy.showSteps)
  const chevron = <ChevronRightIcon className={cn("size-[18px] shrink-0 opacity-70 transition-transform", open ? "rotate-90" : "rtl:rotate-180")} aria-hidden />

  return (
    <div className="space-y-2">
      {done.length > 0 && (
        <button type="button" onClick={() => setOpen(!open)} aria-expanded={open} className={row}>
          <ListIcon className={icon} aria-hidden />
          <span dir="auto" className="min-w-0 truncate">{headline}</span>
          <span dir="ltr" className="shrink-0 rounded-md bg-muted px-1.5 py-0.5 text-[13px] font-medium tabular-nums">{done.length}</span>
          {chevron}
          <span className="sr-only">{toggle}</span>
        </button>
      )}
      {live && (
        <div className={cn(row, "hover:text-muted-foreground")}>
          <ClaudeGlyph className="w-[21px] text-[19px] leading-none" />
          <span dir="ltr" className="machine-shimmer min-w-0 truncate">{live}</span>
        </div>
      )}
      {open && done.length > 0 && (
        <ol className="ms-[10px] list-none space-y-2 border-s ps-4 text-[15px] leading-6 text-muted-foreground md:text-sm">
          {done.map((name, i) => (
            <li key={i} dir="ltr" className="flex items-start gap-2.5 text-start">
              <WrenchIcon className="mt-[0.2em] size-4 shrink-0" aria-hidden />
              <span className="min-w-0">{name}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}

/**
 * Claude Code's status line under a running turn: `✻ Weighing it up… (12s · ↓ 1.4k tokens · esc to interrupt)`.
 * The glyph cycles in terracotta, the verb is fixed for the turn in plain terracotta (no effect), the seconds tick, Esc interrupts.
 * `tokens` is the run's output so far; without it that part is left out.
 */
export function ClaudeStatus({ isAr, seed, tokens = null, onStop }: { isAr: boolean; seed: string; tokens?: number | null; onStop?: () => void }) {
  const t = tr(isAr)
  const [seconds, setSeconds] = useState(0)
  useEffect(() => {
    const start = Date.now()
    const id = window.setInterval(() => setSeconds(Math.floor((Date.now() - start) / 1000)), 1000)
    return () => window.clearInterval(id)
  }, [])
  // Esc interrupts, unless something else (a menu, a chip) already took the key.
  const stopRef = useRef(onStop)
  stopRef.current = onStop
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented) stopRef.current?.()
    }
    window.addEventListener("keydown", on)
    return () => window.removeEventListener("keydown", on)
  }, [])
  const word = turnWord(isAr ? words.ar : words.en, seed)

  return (
    <div role="status" className="flex min-w-0 items-baseline gap-2.5 text-[17px] leading-7 md:text-base">
      <ClaudeGlyph className="text-[1.15em]" />
      <span className="sr-only">{t(copy.working)}</span>
      <span aria-hidden className="min-w-0 truncate">
        <span className="text-claude">{word}</span>
        <span className="tabular-nums text-muted-foreground">
          {" ("}
          {statusDetail(seconds, tokens, isAr, t(copy.interrupt))}
          {")"}
        </span>
      </span>
    </div>
  )
}
