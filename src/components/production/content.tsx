"use client"

import { useEffect, useMemo, useState } from "react"
import { BoxIcon, CameraIcon, CheckIcon, CircleAlertIcon, CircleIcon, ClapperboardIcon, FileIcon, ImageIcon, ShapesIcon } from "lucide-react"

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"
import { tr } from "../chat/config"
import { type FilmData, brandOf, filesExist, filmData, listFilms, thumbOf } from "./actions"
import { copy } from "./config"
import { type Brand, type FilmSummary, type Scene, type SceneKind, defaultFilm, filesFor, gateOf, orderFormula, parseScenes, slotsFor, stageFormula } from "./pure"

const accent = "text-[color:var(--cli-accent)]"
const dim = "text-[color:var(--cli-dim)]"

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
  const [tab, setTab] = useState("stages")

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
  const scenes = useMemo(() => (current ? parseScenes(current.shotList, current.manifest, isAr ? "ar" : "en") : []), [current, isAr])
  if (films.length === 0 || !id || !current) return null

  const film = current.film
  const lang = film.lang ?? (isAr ? "ar" : "en")
  const pick = (formula: string, scene?: Scene) => onPick(formula, slotsFor(film.id, scene))

  return (
    <div dir="ltr" className="relative font-mono text-[12px] leading-5">

        <div
          role="region"
          aria-label={t(copy.label)}
          className="mx-1 mt-2 max-h-[min(50vh,24rem)] overflow-y-auto border-t border-border/60 px-2 pb-1 pt-3"
        >
          <Tabs value={tab} onValueChange={setTab} className="gap-2">
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
              <TabsList className="h-auto w-fit gap-x-4 rounded-none bg-transparent p-0">
                {(["stages", "scenes", "brand"] as const).map((k) => (
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
              {films.length > 1 && (
          <select
            value={id}
            onChange={(e) => setId(e.target.value)}
            aria-label={t(copy.film)}
            className="ms-auto max-w-[40%] shrink-0 truncate rounded bg-transparent py-1 font-sans text-sm text-muted-foreground/70 outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring"
          >
            {films.map((f) => (
              <option key={f.id} value={f.id}>
                {f.slug} {f.version}
              </option>
            ))}
          </select>
        )}
            </div>

            <TabsContent value="stages" className="m-0">
              <ol className="list-none space-y-px">
                {film.stages.map((s) => {
                  const gate = gateOf(film, s.name)
                  const isNext = film.next?.name === s.name
                  return (
                    <li key={s.name}>
                      <button
                        type="button"
                        onClick={() => pick(stageFormula(s.name))}
                        className={cn("flex w-full items-baseline gap-2 rounded px-1.5 py-0.5 text-start transition-colors hover:bg-foreground/5", isNext && accent)}
                      >
                        <span aria-hidden className="w-3 shrink-0">{s.done ? <CheckIcon className="size-3 translate-y-0.5" /> : isNext ? "▸" : <CircleIcon className={cn("size-2.5 translate-y-0.5", dim)} />}</span>
                        <span className={cn("w-5 shrink-0 tabular-nums", !isNext && dim)}>{String(s.n).padStart(2, "0")}</span>
                        <span className={cn("w-24 shrink-0", !isNext && !s.done && dim)}>{s.name}</span>
                        <span className={cn("min-w-0 flex-1 truncate", dim)}>{s.file ?? ""}</span>
                        {s.variants && (
                          <span className={cn("shrink-0 tabular-nums", dim)}>
                            {s.variants.done}/{s.variants.total} {t(copy.variants)}
                          </span>
                        )}
                        {gate && (
                          <span className={cn("shrink-0", gate.signed ? "text-foreground" : dim)}>
                            {gate.name} {gate.signed ? "✔" : "○"}
                          </span>
                        )}
                      </button>
                    </li>
                  )
                })}
              </ol>
            </TabsContent>

            <TabsContent value="scenes" className="m-0">
              {scenes.length === 0 ? (
                <p className={dim}>{t(copy.noScenes)}</p>
              ) : (
                <ScenesList scenes={scenes} lang={lang} isAr={isAr} onPick={(s) => pick(orderFormula(s.kind), s)} />
              )}
            </TabsContent>

            <TabsContent value="brand" className="m-0">
              {tab === "brand" && <BrandView identity={film.identity} isAr={isAr} />}
            </TabsContent>
          </Tabs>
          <p className={cn("mt-2", dim)}>{t(copy.tapHint)}</p>
        </div>


    </div>
  )
}

const KIND_ICON: Record<SceneKind, typeof FileIcon> = {
  capture: CameraIcon,
  "3d": BoxIcon,
  image: ImageIcon,
  video: ClapperboardIcon,
  graphic: ShapesIcon,
  other: FileIcon,
}

/** The scenes: a thumbnail of the first image, the kind icon, the line, and what is missing. */
function ScenesList({ scenes, lang, isAr, onPick }: { scenes: Scene[]; lang: string; isAr: boolean; onPick: (s: Scene) => void }) {
  const t = tr(isAr)
  const paths = useMemo(() => scenes.flatMap((s) => filesFor(s, lang).map((f) => f.path)), [scenes, lang])
  const [exists, setExists] = useState<Map<string, boolean>>(new Map())
  useEffect(() => {
    let live = true
    void filesExist(paths).then((ok) => live && setExists(new Map(paths.map((p, i) => [p, ok[i] ?? true]))))
    return () => {
      live = false
    }
  }, [paths])

  return (
    <ol className="list-none space-y-px">
      {scenes.map((s) => {
        const files = filesFor(s, lang)
        const gone = files.filter((f) => exists.get(f.path) === false)
        const none = files.length === 0
        const Icon = KIND_ICON[s.kind]
        const image = files.find((f) => f.kind === "image" && exists.get(f.path) !== false)
        return (
          <li key={s.id}>
            <button type="button" onClick={() => onPick(s)} className="flex w-full items-center gap-2 rounded px-1.5 py-1 text-start transition-colors hover:bg-foreground/5">
              <Thumb path={image?.path ?? null} />
              <Icon aria-hidden className={cn("size-3.5 shrink-0", dim)} />
              <span className="w-28 shrink-0 truncate">{s.id}</span>
              <span className={cn("w-7 shrink-0 tabular-nums", dim)}>{s.seconds !== null ? `${s.seconds}s` : ""}</span>
              <span dir="auto" className={cn("min-w-0 flex-1 truncate font-sans text-[12px]", dim)}>
                {s.line || s.visual}
              </span>
              {none || gone.length > 0 ? (
                <span className="flex shrink-0 items-center gap-1 text-destructive">
                  <CircleAlertIcon aria-hidden className="size-3" />
                  {none ? t(copy.missing) : `${gone.length}/${files.length} ${t(copy.missing)}`}
                </span>
              ) : (
                <span className={cn("shrink-0 tabular-nums", dim)}>{files.length}</span>
              )}
            </button>
          </li>
        )
      })}
    </ol>
  )
}

/** A 9:16 thumbnail, read through the one checked command; an empty frame while it loads or when there is no image. */
function Thumb({ path }: { path: string | null }) {
  const [src, setSrc] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    setSrc(null)
    if (path) void thumbOf(path).then((s) => live && setSrc(s))
    return () => {
      live = false
    }
  }, [path])
  return (
    <span aria-hidden className="flex h-10 w-[22px] shrink-0 items-center justify-center overflow-hidden rounded-[3px] bg-foreground/5">
      {src && (
        // eslint-disable-next-line @next/next/no-img-element -- a data URL from the checked Rust command
        <img src={src} alt="" className="size-full object-cover" />
      )}
    </span>
  )
}

/** The brand: logo, font names in their own face, colour swatches, voice. Read once per identity. */
function BrandView({ identity, isAr }: { identity: string; isAr: boolean }) {
  const t = tr(isAr)
  const [brand, setBrand] = useState<Brand | null | undefined>(undefined)
  useEffect(() => {
    let live = true
    void brandOf(identity).then((b) => live && setBrand(b))
    return () => {
      live = false
    }
  }, [identity])
  if (brand === undefined) return <p className={dim}>{t(copy.loading)}</p>
  if (!brand) return <p className={dim}>{t(copy.noBrand)}</p>
  const row = "flex items-start gap-3"
  const label = cn("w-14 shrink-0", dim)
  return (
    <div className="space-y-2">
      <div className={row}>
        <span className={label}>{t(copy.logo)}</span>
        {brand.logo ? (
          // eslint-disable-next-line @next/next/no-img-element -- a data URL from the checked Rust command
          <img src={brand.logo} alt={brand.identity} className="h-10 w-auto max-w-40 rounded bg-foreground/5 object-contain p-1" />
        ) : (
          <span>{brand.identity}</span>
        )}
      </div>
      <div className={row}>
        <span className={label}>{t(copy.fonts)}</span>
        <ul className="list-none space-y-0.5">
          {brand.fonts.map((f) => (
            <li key={f.role} className="flex items-baseline gap-2">
              <span className="text-[14px]" style={{ fontFamily: fontStack(f.name) }}>{f.name}</span>
              <span className={dim}>{f.role}</span>
            </li>
          ))}
        </ul>
      </div>
      <div className={row}>
        <span className={label}>{t(copy.palette)}</span>
        <ul className="flex list-none flex-wrap gap-x-3 gap-y-1">
          {brand.palette.map((p) => (
            <li key={p.name} className="flex items-center gap-1.5">
              <span aria-hidden className="size-4 rounded border border-foreground/20" style={{ backgroundColor: p.hex }} />
              <span>{p.hex}</span>
              <span className={dim}>{p.name}</span>
            </li>
          ))}
        </ul>
      </div>
      {brand.voice && (
        <div className={row}>
          <span className={label}>{t(copy.voice)}</span>
          <span>{brand.voice}</span>
        </div>
      )}
    </div>
  )
}

/** The face a font name is shown in: the app's own Thmanyah faces for those names, else the name itself. */
function fontStack(name: string): string {
  if (/thmanyah serif/i.test(name)) return "'thmanyah serif display', serif"
  if (/thmanyah/i.test(name)) return "'thmanyah sans', sans-serif"
  return `'${name}', var(--font-sans)`
}
