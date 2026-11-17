"use client"

// copy of hogwarts next-action-client.tsx — change both together.
// (hogwarts/src/components/school-dashboard/dashboard/next-action-client.tsx)
import { useEffect, useMemo, useRef, useState } from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"
import { tr } from "../chat/config"
import { copy } from "./config"
import { runStart } from "./data"
import type { SpotlightItem } from "./spotlight"

/**
 * The menu's banner: the dashboard's green hero object — the same ground, geometry, type and pills
 * as the Hogwarts next-action card. It states ONE item, the most important to do (or to know), and
 * flips to the next on a timer. Ranking is `rankSpotlight`'s; this component only rotates.
 *
 * A FIXED height. It swaps its sentence every few seconds, and a card that grew and shrank under
 * the reader would shove the rest of the menu up and down on a timer. The headline is pinned to a
 * two-line box, which makes the card's height constant by construction rather than by a magic
 * pixel number, so it survives a change of type size.
 *
 * Literal hexes on purpose. This is a brand ground, not a themed surface: it does NOT invert, and
 * every piece of ink on it is pinned DARK — white on `#00bc6d` measures about 2.5:1 and is
 * unreadable. Nothing here may use `primary-foreground`: on this ground that token is white in
 * light mode and black in dark, which is exactly backwards.
 */

/** How long a fully-revealed sentence holds before it leaves. */
const HOLD_MS = 10_000

/**
 * The leave — the sentence evaporates: the whole line lifts and goes out of focus while the
 * letters fade one after another (the stagger is what makes it read as smoke, not a dissolve).
 * The spread is a TOTAL divided across the letters, so a long line and a short one take the same
 * time to go. Opacity is faster than the blur and lands first: the headline sits in a clipped box,
 * and a blur still carrying ink at the box edge would be cut off in a straight line.
 */
const SMOKE_SPREAD_MS = 300
const SMOKE_FADE_MS = 340
/** The last letter starts a full spread late; the cushion keeps the swap off that exact frame. */
const SMOKE_MS = SMOKE_SPREAD_MS + SMOKE_FADE_MS + 60

/** The entrance — every letter starts as a random one and flips until its turn, left to right. */
const REVEAL_MS_PER_CHAR = 26
const FLIP_MS = 55

/**
 * What the noise is made of, per locale. Latin gibberish inside an Arabic line opens a
 * left-to-right run inside a right-to-left one, and bidi then moves it around as it flips.
 */
const ARABIC_CHARSET = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي"
const LATIN_CHARSET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"

/**
 * One rendered unit: a base letter with any combining marks that belong to it. Splitting on code
 * points would tear a shadda off its letter and render it as a floating accent.
 */
const CLUSTERS = /\P{M}\p{M}*|\p{M}+/gu

interface Piece {
  ch: string
  bold: boolean
}

/**
 * The sentence as a flat list of letters, each knowing whether it carries the weight. Every
 * template OPENS with an emphasised phrase, `**like this**`, with the data's `{mark}` inside it.
 * `{mark}` is substituted INSIDE each segment, after the split, so a title containing asterisks
 * cannot reopen the emphasis. Flat, because both motions run across the whole line.
 */
function toPieces(template: string | undefined, mark: string): Piece[] {
  const clusters = (s: string, bold: boolean): Piece[] =>
    (s.match(CLUSTERS) ?? []).map((ch) => ({ ch, bold }))

  if (!template) return clusters(mark, true)

  if (template.includes("**")) {
    return template.split("**").flatMap((seg, i) => clusters(seg.replace("{mark}", mark), i % 2 === 1))
  }

  const [before, after] = template.split("{mark}")
  if (after === undefined) return clusters(template, false)

  return [...clusters(before + mark, true), ...clusters(after, false)]
}

/** The card shows at most this many — the ranking's own cap. */
const MAX_VISIBLE = 4

export function Banner({
  items,
  lang,
  onNavigate,
  className,
}: {
  items: SpotlightItem[]
  lang: Locale
  /** Called when a pill takes the reader off the menu (Start closes it). */
  onNavigate?: () => void
  className?: string
}) {
  const t = tr(lang === "ar")
  const router = useRouter()
  const [index, setIndex] = useState(0)
  const [leaving, setLeaving] = useState(false)
  const [busy, setBusy] = useState(false)
  /**
   * Items the reader has acknowledged. A LOCAL dismissal and nothing more: it takes the entry off
   * this card for this visit and writes nothing. The next open asks the data again.
   */
  const [dismissed, setDismissed] = useState<string[]>([])

  const live = useMemo(
    () => items.filter((x) => !dismissed.includes(x.id)).slice(0, MAX_VISIBLE),
    [items, dismissed],
  )
  const item = live[index % Math.max(1, live.length)]
  // Keyed on the two STRINGS, not on the objects they came out of: `pieces` feeds an effect that
  // sets state, so an identity that changed every render would restart the scramble on its own
  // output and never settle.
  const template = item?.template ?? ""
  const mark = item?.mark ?? ""
  const pieces = useMemo(() => toPieces(template || undefined, mark), [template, mark])

  const revealMs = pieces.length * REVEAL_MS_PER_CHAR

  /**
   * The cycle. Index advances only inside an effect, so the server and the first client render
   * agree on index 0. The clock starts where the sentence finishes arriving: the reader gets the
   * full hold on a line they can actually read. Reduced motion removes the ANIMATION, not the
   * rotation — the copy still changes on the same clock, it simply cuts instead of evaporating.
   */
  useEffect(() => {
    if (live.length < 2) return
    if (typeof window === "undefined") return

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    const enter = reduce ? 0 : revealMs
    const leave = reduce ? 0 : SMOKE_MS

    let holdTimer: ReturnType<typeof setTimeout> | undefined
    let advanceTimer: ReturnType<typeof setTimeout> | undefined
    let onVisible: (() => void) | undefined

    /**
     * A hidden window still runs timers, and cycling into a page nobody is looking at just lands
     * the reader on an arbitrary entry. So a hidden window does not flip — it waits for the reader
     * and then starts the hold OVER.
     */
    const arm = () => {
      holdTimer = setTimeout(() => {
        if (document.hidden) {
          onVisible = arm
          document.addEventListener("visibilitychange", arm, { once: true })
          return
        }

        if (!reduce) setLeaving(true)
        advanceTimer = setTimeout(() => {
          setIndex((i) => (i + 1) % live.length)
          setLeaving(false)
        }, leave)
      }, enter + HOLD_MS)
    }

    arm()

    return () => {
      if (holdTimer) clearTimeout(holdTimer)
      if (advanceTimer) clearTimeout(advanceTimer)
      if (onVisible) document.removeEventListener("visibilitychange", onVisible)
    }
  }, [live.length, index, revealMs])

  if (!item) return null

  async function start(id: string) {
    setBusy(true)
    try {
      await runStart(id)
      onNavigate?.()
      router.push(`/${lang}/?task=${encodeURIComponent(id)}`)
    } catch {
      setBusy(false)
    }
  }

  return (
    <section className={cn("flex flex-col", className)}>
      <div className="relative isolate flex flex-1 flex-col justify-center overflow-hidden rounded-[36px] bg-[#00bc6d] px-8 py-12 text-[#050505] sm:px-12 lg:min-h-[259px]">
        {/* The marginalia mark at the inline end; hidden below md, where the card stacks and it
            would sit under the headline instead of beside it. */}
        <img
          src="/anthropic/marginalia-media-alpha.svg"
          alt=""
          decoding="async"
          draggable={false}
          aria-hidden="true"
          className="pointer-events-none absolute end-12 hidden size-[150px] object-contain md:block lg:size-[168px]"
        />

        {/* ~7 words over two lines in a ~16ch measure, the emphasis carried by WEIGHT inside an
            otherwise light line rather than by a highlight. thmanyah sans ships 300 and 700 here
            (fetched, see scripts/fetch-thmanyah.mjs); a weight the family lacks would be
            synthesised and lose the face's own drawing. `h-[2.7em]` is exactly two lines of
            `leading-[1.35]` — the only thing between the copy and a card that changes height on a
            timer. `text-balance` splits those two lines to near equal width. */}
        <SortingHeadline key={item.id} pieces={pieces} leaving={leaving} locale={lang} />

        {/* One white pill for the way in, one ghost beside it, both a single word. */}
        <div className="relative mt-7 flex flex-wrap items-center gap-2">
          {item.kind === "start" && item.taskId ? (
            <button type="button" disabled={busy} className={pill("default")} onClick={() => void start(item.taskId!)}>
              {t(copy.start)}
            </button>
          ) : item.href ? (
            <Link className={pill("default")} href={`/${lang}${item.href}`}>
              {t(item.kind === "continue" ? copy.continue : copy.pillOpen)}
            </Link>
          ) : null}
          <button
            type="button"
            className={pill("ghost")}
            onClick={() => {
              setDismissed((d) => [...d, item.id])
              setIndex(0)
              setLeaving(false)
            }}
          >
            {t(copy.acknowledge)}
          </button>
        </div>
      </div>
    </section>
  )
}

/**
 * White with dark ink on the green ground, pinned like everything else on this banner so it reads
 * the same in both themes. A token pair would invert in dark mode and lose the contrast the ground
 * was chosen for.
 */
function pill(variant: "default" | "ghost") {
  return cn(
    "inline-flex h-10 items-center justify-center gap-2 rounded-full px-5",
    "text-sm font-medium whitespace-nowrap transition-colors disabled:opacity-60",
    "outline-none focus-visible:ring-2 focus-visible:ring-[#050505]/40",
    variant === "default"
      ? "bg-white text-[#050505] hover:bg-white/90"
      : "text-[#050505]/75 hover:bg-[#050505]/10 hover:text-[#050505]",
  )
}

/**
 * The headline, and both of its motions. Remounted per item (the caller keys it), so the scramble
 * restarts by construction rather than by resetting refs.
 *
 * Every letter is its own span. Arabic still joins across them — shaping runs over the inline run,
 * not per element — and the two properties animated here, `opacity` and `filter`, do not affect
 * it. `display: inline-block` DOES break the joining, which is why the lift is one `translate` on
 * the block and never a per-letter one.
 */
function SortingHeadline({ pieces, leaving, locale }: { pieces: Piece[]; leaving: boolean; locale: string }) {
  const charset = locale === "en" ? LATIN_CHARSET : ARABIC_CHARSET
  const plain = pieces.map((p) => p.ch).join("")

  // Server render and first client render are the real sentence — the noise is seeded in an
  // effect, after hydration, so `Math.random` never reaches the markup the two must agree on.
  const [revealed, setRevealed] = useState(pieces.length)
  const noise = useRef<string[]>([])
  const [, repaint] = useState(0)

  useEffect(() => {
    if (typeof window === "undefined") return
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return

    const pick = () => charset.charAt(Math.floor(Math.random() * charset.length))
    const scramble = () => {
      noise.current = pieces.map((p) => (p.ch === " " ? " " : pick()))
    }

    scramble()
    setRevealed(0)

    const start = performance.now()
    let lastFlip = start
    let frame = 0

    const step = (now: number) => {
      const done = Math.min(pieces.length, Math.floor((now - start) / REVEAL_MS_PER_CHAR))
      setRevealed(done)
      if (done >= pieces.length) return

      if (now - lastFlip >= FLIP_MS) {
        for (let i = done; i < pieces.length; i += 1) {
          noise.current[i] = pieces[i].ch === " " ? " " : pick()
        }
        lastFlip = now
        // `revealed` only changes every REVEAL_MS_PER_CHAR, so on its own it would hold the paint
        // still between steps and the noise would sit frozen.
        repaint((n) => n + 1)
      }

      frame = requestAnimationFrame(step)
    }

    frame = requestAnimationFrame(step)
    return () => cancelAnimationFrame(frame)
  }, [pieces, charset])

  // Divided across the sentence, so the leave takes the same time whatever the copy is.
  const stagger = SMOKE_SPREAD_MS / Math.max(1, pieces.length)

  return (
    <h2
      aria-label={plain}
      className={cn(
        "relative h-[2.7em] max-w-[16ch] overflow-hidden text-3xl leading-[1.35] font-light text-balance sm:text-4xl lg:max-w-[420px] lg:text-[38px]",
        // `translate`, not `transform` — Tailwind v4 sets the standalone CSS property, so a
        // transition list naming `transform` animates the blur and snaps the lift.
        "transition-[filter,translate] duration-500 ease-out motion-reduce:transition-none",
        leaving ? "-translate-y-1.5 blur-[6px]" : "translate-y-0 blur-none",
      )}
      style={{ fontFamily: '"thmanyah sans", sans-serif' }}
    >
      <span aria-hidden="true">
        {pieces.map((piece, i) => (
          <span
            key={i}
            className={cn(
              "transition-opacity ease-in motion-reduce:transition-none",
              piece.bold && "font-bold",
              leaving ? "opacity-0" : "opacity-100",
            )}
            style={{
              transitionDuration: `${SMOKE_FADE_MS}ms`,
              transitionDelay: leaving ? `${Math.round(i * stagger)}ms` : "0ms",
            }}
          >
            {i < revealed ? piece.ch : (noise.current[i] ?? piece.ch)}
          </span>
        ))}
      </span>
    </h2>
  )
}
