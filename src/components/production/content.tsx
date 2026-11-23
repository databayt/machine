"use client"

import { useEffect, useRef, useState } from "react"
import { LockIcon } from "lucide-react"

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"
import { stationCompose, stationSettings } from "../pack/actions"
import { tr } from "../chat/config"
import { filmData, listFilms, lockStage, type FilmData } from "./actions"
import { copy } from "./config"
import { type FilmSummary, type Light, STAGES, defaultFilm, defaultSettings, gateOf, lightOf } from "./pure"
import {
  type Composed,
  FALLBACK_SETTINGS,
  type L,
  type SettingGroup,
  type SettingsDoc,
  composeLocal,
  initialPicks,
  parseCompose,
  parseSettings,
  pickKey,
  visibleOptions,
  withoutNew,
} from "./settings-pure"

/**
 * The settings panel above the chat box (the gear). Its tabs come from data (`station settings`): Film, Kind,
 * Audience, Format, Style, Stages; an older kun falls back to the choices hard-coded here. A choice on a NEW film only
 * composes the request (the composer shows it and sends it); on an open film it re-locks the stage that owns it.
 * Stage cards never fill the box: a tap brings the stage into the chat (`onStage`). Nothing here runs a model.
 */
export function ProductionStrip({
  isAr,
  repo,
  busy,
  onStage,
  onFilm,
  open,
  onOpenChange,
  onSettings,
  onCompose,
  refresh,
}: {
  isAr: boolean
  repo: string
  /** A run is going: the status is read again when it ends. */
  busy: boolean
  /** A stage card was tapped: the chat shows that stage (no model run). */
  onStage: (stage: string) => void
  /** The selected film (null: a new film), so the chat can fill `project` for formulas that typed text reaches. */
  onFilm: (id: string | null) => void
  /** The settings panel is open: the composer's gear toggles it (the /social/publish pattern). */
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The choices' slots (without the reserved `new`), whenever they change: every later fill carries them. */
  onSettings: (slots: Record<string, string> | null) => void
  /** What the choices compose now (a formula with its filled slots, or no formula): the composer shows it. */
  onCompose: (composed: Composed | null) => void
  /** The chat changed the film's ledger (a script lock): read the settings and status again. */
  refresh: number
}) {
  const t = tr(isAr)
  const l = (x: L) => (isAr ? x.ar : x.en)
  const [films, setFilms] = useState<FilmSummary[]>([])
  const [id, setId] = useState<string | null>(null)
  /** The person chose "New film" on purpose: a refreshed list keeps it. */
  const chosenNew = useRef(false)
  const seen = useRef(-1)
  const [data, setData] = useState<FilmData | null>(null)
  const [tab, setTab] = useState<string>("stages")
  const tabInit = useRef(false)
  const [reload, setReload] = useState(0)

  // The films of the repo: read on mount, when the repo changes and when a run ends.
  useEffect(() => {
    if (busy) return
    let live = true
    void listFilms(repo || null).then((list) => {
      if (!live) return
      setFilms(list)
      // A film created since the last read (the plan was accepted) becomes the selected one.
      if (seen.current >= 0 && list.length > seen.current) chosenNew.current = false
      seen.current = list.length
      setId((cur) => (chosenNew.current ? null : defaultFilm(list, cur)))
    })
    return () => {
      live = false
    }
  }, [repo, busy, refresh])

  useEffect(() => {
    onFilm(id)
  }, [id, onFilm])

  // The selected film's status, shot list and manifest.
  useEffect(() => {
    if (!id) return setData(null)
    if (busy) return
    let live = true
    void filmData(id).then((d) => live && setData(d?.film.id === id ? d : null))
    return () => {
      live = false
    }
  }, [id, busy, refresh, reload])

  const current = data && data.film.id === id ? data : null
  const film = current?.film ?? null
  // Esc closes the panel, as in the CLI.
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onOpenChange(false)
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [open, onOpenChange])

  // The tabs and choices: the station's, else the ones hard-coded today. `picks` are the choice values on screen.
  const [doc, setDoc] = useState<SettingsDoc>(FALLBACK_SETTINGS)
  const [fallback, setFallback] = useState(true)
  const [picks, setPicks] = useState<Record<string, string>>({})
  const picksRef = useRef(picks)
  picksRef.current = picks
  const filmVariants = film?.variants.join(",") ?? ""
  useEffect(() => {
    if (busy) return
    let live = true
    void stationSettings("video", repo || null, id).then((raw) => {
      if (!live) return
      const parsed = parseSettings(raw)
      const d = parsed ?? FALLBACK_SETTINGS
      const hasFilm = !!id
      // Saved on this computer: a new film's choices; an open film's only when a lock was refused (or an older kun).
      const saved = readSaved(pickKey(id))
      const base = !parsed && id ? { ...defaultSettings(filmVariants ? filmVariants.split(",") : []) } : {}
      const keepKind = hasFilm && picksRef.current.kind ? { kind: picksRef.current.kind } : {}
      setDoc(d)
      setFallback(!parsed)
      setPicks(initialPicks(d, hasFilm, { ...base, ...saved, ...keepKind }))
    })
    return () => {
      live = false
    }
  }, [repo, id, busy, refresh, reload, filmVariants])

  // The first look: the Film tab when there is no film to work on, else the stages.
  useEffect(() => {
    if (tabInit.current || seen.current < 0) return
    tabInit.current = true
    if (films.length === 0) setTab("film")
  }, [films])
  const tabs = doc.tabs
  const shownTab = tabs.some((x) => x.id === tab) ? tab : (tabs[0]?.id ?? "film")

  // Every change of the choices composes the request again; its slots (never `new`) go to every later fill.
  const onComposeRef = useRef(onCompose)
  onComposeRef.current = onCompose
  const onSettingsRef = useRef(onSettings)
  onSettingsRef.current = onSettings
  useEffect(() => {
    if (Object.keys(picks).length === 0) return
    let live = true
    void (async () => {
      let c: Composed | null = null
      if (!fallback) c = parseCompose(await stationCompose("video", repo || null, id, picks))
      if (!c) c = composeLocal(fallback ? doc : FALLBACK_SETTINGS, picks, id)
      if (!live) return
      onComposeRef.current(c)
      onSettingsRef.current(withoutNew(c.set))
    })()
    return () => {
      live = false
    }
  }, [picks, fallback, doc, repo, id])

  const choose = (g: SettingGroup, value: string) => {
    const next = { ...picks, [g.key]: value }
    setPicks(next)
    if (!id) return writeSaved(pickKey(null), next)
    if (g.key === "kind") return
    const local = () => writeSaved(pickKey(id), next)
    // An open film's choice re-locks the stage that owns it; a kun that cannot falls back to saving it here.
    if (fallback || !g.stage) return local()
    lockStage(id, g.stage, { set: { [g.key]: value } }).then(() => setReload((n) => n + 1), local)
  }

  return (
    <div dir="ltr" className="relative leading-5">
      <div role="region" aria-label={t(copy.label)} className="mx-1 mt-2 max-h-[min(50vh,24rem)] overflow-y-auto border-t border-border/60 px-2 pb-1 pt-3">
        <Tabs value={shownTab} onValueChange={setTab} className="gap-3">
          <TabsList className="h-auto w-fit flex-wrap gap-x-4 gap-y-1 rounded-none bg-transparent p-0">
            {tabs.map((x) => (
              <TabsTrigger
                key={x.id}
                value={x.id}
                // Keep focus in the box: Radix switches on mousedown and skips a prevented event, so switch here.
                onMouseDown={(e) => {
                  e.preventDefault()
                  setTab(x.id)
                }}
                className="h-auto flex-none cursor-pointer rounded-none border-0 bg-transparent px-0 py-1 font-sans text-sm font-normal text-muted-foreground/70 shadow-none transition-colors duration-150 hover:text-foreground data-[state=active]:bg-transparent data-[state=active]:font-medium data-[state=active]:text-foreground data-[state=active]:shadow-none"
              >
                {l(x.label)}
              </TabsTrigger>
            ))}
          </TabsList>

          {tabs.map((x) => (
            <TabsContent key={x.id} value={x.id} className="m-0">
              {x.id === "stages" ? (
                <Cards>
                  {(film ? film.stages.map((s) => s.name) : [...STAGES]).map((name, i) => {
                    const s = film?.stages.find((y) => y.name === name)
                    const gate = film ? gateOf(film, name) : null
                    const isNext = film?.next?.name === name
                    const light = s ? lightOf(s, isNext) : "grey"
                    return (
                      <Card key={name} active={isNext} disabled={!film} onClick={() => onStage(name)}>
                        <span className="flex items-center justify-between font-mono text-[11px] text-muted-foreground">
                          <span className="tabular-nums">{String(s?.n ?? i + 1).padStart(2, "0")}</span>
                          <span className="flex items-center gap-1.5">
                            {s?.locked && <LockIcon role="img" aria-label={t(copy.locked)} className="size-3" />}
                            <span role="img" aria-label={t(copy[light])} className={cn("size-2 rounded-full", LIGHT[light])} />
                          </span>
                        </span>
                        <span className="text-sm font-medium capitalize text-foreground">{name}</span>
                        <span className="mt-auto text-xs text-muted-foreground">
                          {s ? (s.variants ? `${s.variants.done}/${s.variants.total} ${t(copy.variants)}` : s.done ? t(copy.done) : t(copy.todo)) : ""}
                          {gate && ` · ${gate.name} ${gate.signed ? "✔" : "○"}`}
                        </span>
                      </Card>
                    )
                  })}
                </Cards>
              ) : x.source ? (
                <Cards>
                  <Card
                    active={id === null}
                    onClick={() => {
                      chosenNew.current = true
                      setId(null)
                    }}
                  >
                    <span className="font-mono text-[11px] text-muted-foreground">+</span>
                    <span className="truncate text-sm font-medium text-foreground">{t(copy.newFilm)}</span>
                    <span className="mt-auto truncate text-xs text-muted-foreground">{t(copy.newFilmHint)}</span>
                  </Card>
                  {films.map((f) => {
                    const on = f.id === id
                    const done = on && film ? film.stages.filter((s) => s.done).length : null
                    return (
                      <Card
                        key={f.id}
                        active={on}
                        onClick={() => {
                          chosenNew.current = false
                          setId(f.id)
                        }}
                      >
                        <span className="font-mono text-[11px] text-muted-foreground">{f.version}</span>
                        <span className="truncate text-sm font-medium text-foreground">{f.slug}</span>
                        <span className="mt-auto text-xs text-muted-foreground">
                          {on && done !== null && film ? `${done}/${film.stages.length} · ${film.next ? film.next.name : t(copy.done)}` : f.next ?? ""}
                        </span>
                      </Card>
                    )
                  })}
                </Cards>
              ) : (
                <div className="space-y-2.5 font-sans">
                  {x.groups.map((g) => (
                    <div key={g.key} className="flex items-center gap-3">
                      <span className="w-20 shrink-0 text-xs text-muted-foreground">{l(g.label)}</span>
                      <div className="flex flex-wrap gap-1.5">
                        {visibleOptions(g, !!id).map((o) => (
                          <button
                            key={o.value}
                            type="button"
                            aria-pressed={picks[g.key] === o.value}
                            title={o.hint ? l(o.hint) : undefined}
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => choose(g, o.value)}
                            className={cn(
                              "rounded-full px-3 py-1 text-sm transition-colors duration-150",
                              picks[g.key] === o.value ? "bg-foreground text-background" : "bg-muted text-muted-foreground hover:text-foreground",
                            )}
                          >
                            {l(o.label)}
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </TabsContent>
          ))}
        </Tabs>
      </div>
    </div>
  )
}

function readSaved(key: string): Record<string, string> {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(key) ?? "null")
    return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, string>) : {}
  } catch {
    return {}
  }
}

function writeSaved(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {}
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

function Card({ active, disabled, onClick, children }: { active?: boolean; disabled?: boolean; onClick: () => void; children: React.ReactNode }) {
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
      disabled={disabled}
      className={cn(
        "flex h-24 w-36 shrink-0 snap-start flex-col gap-1 rounded-2xl bg-muted p-3 text-start transition-colors duration-150 hover:bg-foreground/[0.07]",
        active && "bg-foreground/[0.08]",
        disabled && "pointer-events-none opacity-40",
      )}
    >
      {children}
    </button>
  )
}
