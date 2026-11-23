"use client"

import { useEffect, useRef, useState } from "react"

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"
import { tr } from "../chat/config"
import { filmData, listFilms, type FilmData } from "./actions"
import { copy } from "./config"
import { type FilmSummary, type Light, defaultFilm, gateOf, lightOf, slotsFor, stageFormula } from "./pure"


/**
 * The production line above the chat box. Collapsed it is one quiet line: `▸ intro-cinematic · ●●●●●●○○○○○○○○ 07 visual
 * · script ✔ look ○` with a film select when there are several. Open it grows upward over the transcript (the box never
 * moves) with three tabs: Stages, Scenes, Brand. It has no buttons that run anything: a row only fills the chat box with
 * its formula (`onPick`), and the chat stays the only place work runs.
 *
 * Shows nothing until the line has a film for this repo, and nothing at all when the line cannot be read.
 */
export function ProductionStrip({
  isAr,
  repo,
  busy,
  onPick,
  onFilm,
  open,
  onOpenChange,
  onAvailable,
}: {
  isAr: boolean
  repo: string
  /** A run is going: the status is read again when it ends. */
  busy: boolean
  /** A row was tapped: the formula to fill and the slots it hands to `fill` (always the film as `project`). */
  onPick: (formula: string, set: Record<string, string>) => void
  /** The selected film, so the chat can fill `project` for formulas that typed text reaches. */
  onFilm: (id: string | null) => void
  /** The settings panel is open: the composer's gear toggles it (the /social/publish pattern). */
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Whether there is a film to show, so the composer shows its gear only then. */
  onAvailable: (available: boolean) => void
}) {
  const t = tr(isAr)
  const [films, setFilms] = useState<FilmSummary[]>([])
  const [id, setId] = useState<string | null>(null)
  const [data, setData] = useState<FilmData | null>(null)
  const [tab, setTab] = useState<string>("stages")

  // The films of the repo: read on mount, when the repo changes and when a run ends.
  useEffect(() => {
    if (busy) return
    let live = true
    void listFilms(repo || null).then((list) => {
      if (!live) return
      setFilms(list)
      setId((cur) => defaultFilm(list, cur))
    })
    return () => {
      live = false
    }
  }, [repo, busy])

  useEffect(() => {
    onFilm(id)
  }, [id, onFilm])

  // The selected film's status, shot list and manifest.
  useEffect(() => {
    if (!id || busy) return
    let live = true
    void filmData(id).then((d) => live && setData(d?.film.id === id ? d : null))
    return () => {
      live = false
    }
  }, [id, busy])

  const current = data && data.film.id === id ? data : null
  // Esc closes the panel, as in the CLI.
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onOpenChange(false)
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [open, onOpenChange])
  const available = films.length > 0 && !!id && !!current
  useEffect(() => {
    onAvailable(available)
    if (!available) onOpenChange(false)
  }, [available, onAvailable, onOpenChange])
  if (films.length === 0 || !id || !current) return null

  const film = current.film
  const pick = (formula: string) => onPick(formula, slotsFor(film.id))

  return (
    <div dir="ltr" className="relative leading-5">

        <div
          role="region"
          aria-label={t(copy.label)}
          className="mx-1 mt-2 max-h-[min(50vh,24rem)] overflow-y-auto border-t border-border/60 px-2 pb-1 pt-3"
        >
          <Tabs value={tab} onValueChange={setTab} className="gap-3">
            <TabsList className="h-auto w-fit flex-wrap gap-x-4 gap-y-1 rounded-none bg-transparent p-0">
              {(["stages", "film"] as const).map((k) => (
                <TabsTrigger
                  key={k}
                  value={k}
                  onMouseDown={(e) => e.preventDefault()}
                  className="h-auto flex-none cursor-pointer rounded-none border-0 bg-transparent px-0 py-1 font-sans text-sm font-normal text-muted-foreground/70 shadow-none transition-colors duration-150 hover:text-foreground data-[state=active]:bg-transparent data-[state=active]:font-medium data-[state=active]:text-foreground data-[state=active]:shadow-none"
                >
                  {t(copy[k])}
                </TabsTrigger>
              ))}
            </TabsList>

            <TabsContent value="stages" className="m-0">
              <Cards>
                {film.stages.map((s) => {
                  const gate = gateOf(film, s.name)
                  const isNext = film.next?.name === s.name
                  const light = lightOf(s, isNext)
                  return (
                    <Card key={s.name} active={isNext} onClick={() => pick(stageFormula(s.name))}>
                      <span className="flex items-center justify-between font-mono text-[11px] text-muted-foreground">
                        <span className="tabular-nums">{String(s.n).padStart(2, "0")}</span>
                        <span role="img" aria-label={t(copy[light])} className={cn("size-2 rounded-full", LIGHT[light])} />
                      </span>
                      <span className="text-sm font-medium capitalize text-foreground">{s.name}</span>
                      <span className="mt-auto text-xs text-muted-foreground">
                        {s.variants ? `${s.variants.done}/${s.variants.total} ${t(copy.variants)}` : s.done ? t(copy.done) : t(copy.todo)}
                        {gate && ` · ${gate.name} ${gate.signed ? "✔" : "○"}`}
                      </span>
                    </Card>
                  )
                })}
              </Cards>
            </TabsContent>

            <TabsContent value="film" className="m-0">
              <Cards>
                {films.map((f) => {
                  const on = f.id === id
                  const done = on ? film.stages.filter((s) => s.done).length : null
                  return (
                    <Card key={f.id} active={on} onClick={() => setId(f.id)}>
                      <span className="font-mono text-[11px] text-muted-foreground">{f.version}</span>
                      <span className="truncate text-sm font-medium text-foreground">{f.slug}</span>
                      <span className="mt-auto text-xs text-muted-foreground">
                        {on && done !== null ? `${done}/${film.stages.length} · ${film.next ? film.next.name : t(copy.done)}` : f.next ?? ""}
                      </span>
                    </Card>
                  )
                })}
              </Cards>
            </TabsContent>
          </Tabs>
        </div>


    </div>
  )
}

const LIGHT: Record<Light, string> = {
  green: "bg-emerald-500",
  orange: "bg-orange-500",
  red: "bg-red-500",
  grey: "bg-foreground/20",
}

/** A horizontal row of cards that scrolls sideways, as on /social/publish. */
function Cards({ children }: { children: React.ReactNode }) {
  return <div className="-mx-1 flex snap-x gap-2 overflow-x-auto px-1 pb-1 font-sans [scrollbar-width:none]">{children}</div>
}

function Card({ active, onClick, children }: { active?: boolean; onClick: () => void; children: React.ReactNode }) {
  // The next stage scrolls into view in the row.
  const ref = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ inline: "center", block: "nearest" })
  }, [active])
  return (
    <button
      ref={ref}
      type="button"
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
      className={cn(
        "flex h-24 w-36 shrink-0 snap-start flex-col gap-1 rounded-2xl bg-muted p-3 text-start transition-colors duration-150 hover:bg-foreground/[0.07]",
        active && "bg-foreground/[0.08]",
      )}
    >
      {children}
    </button>
  )
}
