"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { ChevronDownIcon, PlusIcon, XIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"

import { tr } from "../chat/config"
import { OpenInMachine, useInMachine } from "../shell/content"
import { Logo } from "../plugins/card"
import { SHELL_TOP } from "../shell/grid"
import {
  decode,
  onTerminalData,
  onTerminalExit,
  onTerminalSessions,
  terminalAttach,
  terminalClose,
  terminalLanes,
  terminalList,
  terminalOpen,
  terminalRename,
  terminalResize,
  terminalSetActive,
  terminalFullscreen,
  terminalWrite,
  type SessionInfo,
  type TerminalLane,
  type TerminalList,
} from "./actions"
import { copy } from "./config"
import { freshPart } from "./pure"

import "@fontsource-variable/jetbrains-mono"
import "@xterm/xterm/css/xterm.css"
import "./scrollbar.css"

/** JetBrains' Terminal tool window, measured off its screenshot. */
const HEADER_BG = "#f7f8fa"
const HEADER_BORDER = "#ebecf0"
const ACCENT = "#3574f0"
const MUTED = "#6c707e"
const FONT = '"JetBrains Mono Variable", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace'

/** Light theme. Every ANSI colour is set, not just the background: the defaults (yellow, white,
 *  bright cyan…) are made for dark and vanish on white, and the agents colour a lot. */
const LIGHT = {
  background: "#ffffff",
  foreground: "#1f2328",
  cursor: "#1f2328",
  cursorAccent: "#ffffff",
  selectionBackground: "#0969da33",
  black: "#24292f",
  red: "#cf222e",
  green: "#116329",
  yellow: "#7d4e00",
  blue: "#0969da",
  magenta: "#8250df",
  cyan: "#1b7c83",
  white: "#6e7781",
  brightBlack: "#57606a",
  brightRed: "#a40e26",
  brightGreen: "#1a7f37",
  brightYellow: "#633c01",
  brightBlue: "#218bff",
  brightMagenta: "#a475f9",
  brightCyan: "#3192aa",
  brightWhite: "#8c959f",
  // shadcn ScrollArea thumb: bg-border (oklch 0.922 0 0 ≈ #e5e5e5), a step darker on hover/drag.
  scrollbarSliderBackground: "#e5e5e5",
  scrollbarSliderHoverBackground: "#d4d4d4",
  scrollbarSliderActiveBackground: "#a3a3a3",
}

/** The grid the last visible terminal had: a new session starts at that size, not 80x24. */
let lastGrid = { cols: 120, rows: 32 }

/** The sessions, live: the Rust registry on mount, then every `terminal://sessions` event. */
function useSessions(enabled: boolean): TerminalList | null {
  const [list, setList] = useState<TerminalList | null>(null)
  useEffect(() => {
    if (!enabled) return
    let dead = false
    let off: (() => void) | undefined
    ;(async () => {
      off = await onTerminalSessions((l) => !dead && setList(l))
      if (dead) return off()
      const first = await terminalList().catch(() => ({ sessions: [], active: null }))
      // An event that raced the first read is newer; keep it.
      if (!dead) setList((cur) => cur ?? first)
    })()
    return () => {
      dead = true
      off?.()
    }
  }, [enabled])
  return list
}

/** One xterm bound to one session. Stays mounted (hidden) while another tab is in front: attach
 *  replays the scrollback, then the live chunks follow without a gap or a repeat. */
function TerminalView({ session, active, ended }: { session: SessionInfo; active: boolean; ended: string }) {
  const id = session.id
  const host = useRef<HTMLDivElement>(null)
  const activeRef = useRef(active)
  const refit = useRef<() => void>(() => {})
  const alive = useRef(session.alive)

  useEffect(() => {
    activeRef.current = active
    if (active) requestAnimationFrame(() => refit.current())
  }, [active])

  useEffect(() => {
    let dead = false
    const cleanups: (() => void)[] = []
    ;(async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit")])
      // The cell size is measured when the terminal opens; the font has to be there by then.
      await document.fonts.load('16px "JetBrains Mono Variable"').catch(() => [])
      const el = host.current
      if (dead || !el) return
      const term = new Terminal({
        cursorBlink: true,
        fontSize: 16,
        lineHeight: 1.3,
        fontFamily: FONT,
        theme: LIGHT,
        allowProposedApi: false,
      })
      const fit = new FitAddon()
      term.loadAddon(fit)
      term.open(el)
      cleanups.push(() => term.dispose())

      // Listen first, replay second: chunks that land meanwhile wait, and `seen` drops the overlap.
      let seen = 0
      let ready = false
      const waiting: Parameters<Parameters<typeof onTerminalData>[1]>[0][] = []
      const take = (c: (typeof waiting)[number]) => {
        const bytes = decode(c.data)
        const f = freshPart(c, bytes, seen)
        if (!f) return
        seen = f.seen
        term.write(f.bytes)
      }
      const offData = await onTerminalData(id, (c) => (ready ? take(c) : waiting.push(c)))
      const offExit = await onTerminalExit(id, (code) => {
        alive.current = false
        term.write(`\r\n\x1b[2m[${ended}${code === null ? "" : ` · ${code}`}]\x1b[0m\r\n`)
      })
      cleanups.push(offData, offExit)
      if (dead) return cleanups.forEach((c) => c())

      try {
        const a = await terminalAttach(id)
        if (dead) return cleanups.forEach((c) => c())
        term.write(decode(a.data))
        seen = a.total
      } catch (e) {
        term.write(`\x1b[31m${String(e instanceof Error ? e.message : e)}\x1b[0m\r\n`)
      }
      ready = true
      waiting.splice(0).forEach(take)
      if (!alive.current) term.write(`\r\n\x1b[2m[${ended}]\x1b[0m\r\n`)

      const input = term.onData((d) => void terminalWrite(id, d).catch(() => {}))
      // Only the tab in front sizes the pty: a hidden one measures zero.
      const push = () => {
        if (!activeRef.current || term.cols < 2) return
        lastGrid = { cols: term.cols, rows: term.rows }
        void terminalResize(id, term.cols, term.rows).catch(() => {})
      }
      const size = term.onResize(push)
      cleanups.push(() => input.dispose(), () => size.dispose())
      refit.current = () => {
        try {
          if (el.clientWidth > 0) fit.fit()
          push()
        } catch {}
      }
      const ro = new ResizeObserver(() => refit.current())
      ro.observe(el)
      cleanups.push(() => ro.disconnect())
      refit.current()
      if (activeRef.current) term.focus()
    })()
    return () => {
      dead = true
      cleanups.forEach((c) => c())
    }
    // The session is fixed for the life of this view.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // The terminal is always left-to-right, whatever the page's direction. 12px inside, nothing outside.
  return <div ref={host} dir="ltr" className={cn("kun-terminal min-h-0 flex-1 overflow-hidden bg-white p-3", !active && "hidden")} />
}

/** A flat header icon button. */
function Icon({ label, onClick, children }: { label: string; onClick?: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex size-7 items-center justify-center rounded-md hover:bg-black/5 data-[state=open]:bg-black/5"
      style={{ color: MUTED }}
    >
      {children}
    </button>
  )
}

/** The tab's name, edited in place: Enter saves, Esc cancels, leaving the field saves. */
function NameField({ session, onDone }: { session: SessionInfo; onDone: (name: string | null) => void }) {
  const done = useRef(false)
  const finish = (name: string | null) => {
    if (done.current) return
    done.current = true
    onDone(name)
  }
  return (
    <input
      autoFocus
      defaultValue={session.name}
      maxLength={40}
      aria-label={session.name}
      onFocus={(e) => e.currentTarget.select()}
      onKeyDown={(e) => {
        if (e.key === "Enter") finish(e.currentTarget.value)
        if (e.key === "Escape") finish(null)
      }}
      onBlur={(e) => finish(e.currentTarget.value)}
      className="h-7 w-32 rounded border bg-white px-1.5 text-[15px] outline-none"
      style={{ borderColor: ACCENT }}
    />
  )
}

/** The launchers as cards, in this order; the logo is each product's original mark. */
const CARD_ORDER = ["c", "x", "a", "o", "h", "f"]
const LOGOS: Record<string, string> = {
  c: "/logos/claude-code.svg",
  x: "/logos/openai.svg", // Codex — OpenAI's own mark (no separate Codex logo in public/logos)
  a: "/logos/antigravity.svg",
  o: "/logos/opencode.svg",
  h: "/logos/hermes.png",
  f: "/logos/claude-code.svg",
}
const logoOf = (lane: string): string | undefined => LOGOS[lane]
const NO_BLURB = { en: "", ar: "" }

/**
 * JetBrains' Terminal tool window: header (label, tabs, +, ⌄, ×) over a flush white body. It covers
 * the whole app window; × hands control back to the cards page (sessions keep running).
 */
function TerminalTool({ lang, onClose }: { lang: Locale; onClose: () => void }) {
  const t = tr(lang === "ar")
  const list = useSessions(true)
  const [lanes, setLanes] = useState<TerminalLane[] | null>(null)
  const [editing, setEditing] = useState<string | null>(null)
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    terminalLanes().then(setLanes).catch(() => setLanes([]))
  }, [])
  useEffect(() => {
    if (!menu) return
    const off = () => setMenu(null)
    const key = (e: KeyboardEvent) => e.key === "Escape" && off()
    window.addEventListener("click", off)
    window.addEventListener("blur", off)
    window.addEventListener("keydown", key)
    return () => {
      window.removeEventListener("click", off)
      window.removeEventListener("blur", off)
      window.removeEventListener("keydown", key)
    }
  }, [menu])

  const sessions = list?.sessions ?? []
  const active = list?.active ?? null

  // The last session closed: nothing to show full screen.
  useEffect(() => {
    if (list && list.sessions.length === 0) onClose()
  }, [list, onClose])

  const open = useCallback(async (lane: string | null) => {
    setError(null)
    try {
      await terminalOpen(lane, lastGrid.cols, lastGrid.rows)
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e))
    }
  }, [])

  const rename = (id: string, name: string | null) => {
    setEditing(null)
    if (name !== null) void terminalRename(id, name).catch(() => {})
  }

  const flat = "flex size-7 items-center justify-center rounded-md hover:bg-black/5 data-[state=open]:bg-black/5"
  return (
    <section aria-label={t(copy.title)} dir="ltr" className="flex h-full min-h-0 min-w-0 flex-col bg-white text-[#1f2328]">
      <header
        className="flex h-12 shrink-0 items-stretch border-b ps-5 pe-3 text-[15px]"
        style={{ background: HEADER_BG, borderColor: HEADER_BORDER }}
      >
        <span className="flex items-center pe-5 font-semibold">{t(copy.title)}</span>
        <div role="tablist" className="flex min-w-0 items-stretch gap-1 overflow-x-auto">
          {sessions.map((s) => {
            const on = s.id === active
            return (
              <div
                key={s.id}
                role="presentation"
                className="group relative flex shrink-0 items-center gap-1.5 px-3"
                onContextMenu={(e) => {
                  e.preventDefault()
                  setMenu({ id: s.id, x: e.clientX, y: e.clientY })
                }}
              >
                {editing === s.id ? (
                  <NameField session={s} onDone={(n) => rename(s.id, n)} />
                ) : (
                  <button
                    role="tab"
                    aria-selected={on}
                    onClick={() => void terminalSetActive(s.id).catch(() => {})}
                    onDoubleClick={() => setEditing(s.id)}
                    className={cn("max-w-48 truncate", !s.alive && "opacity-50")}
                  >
                    {s.name}
                  </button>
                )}
                <button
                  type="button"
                  aria-label={`${t(copy.close)} ${s.name}`}
                  onClick={() => void terminalClose(s.id).catch(() => {})}
                  className="flex size-4 items-center justify-center rounded-sm hover:bg-black/10"
                  style={{ color: "#a8abb5" }}
                >
                  <XIcon className="size-3" />
                </button>
                {on && <span aria-hidden className="absolute inset-x-0 bottom-0 h-0.5 rounded-t-full" style={{ background: ACCENT }} />}
              </div>
            )
          })}
        </div>
        <div className="flex items-center gap-0.5 ps-1" style={{ color: MUTED }}>
          <Icon label={t(copy.add)} onClick={() => void open(null)}>
            <PlusIcon className="size-4" />
          </Icon>
          <DropdownMenu dir={lang === "ar" ? "rtl" : "ltr"}>
            <DropdownMenuTrigger asChild>
              <button type="button" aria-label={t(copy.choose)} title={t(copy.choose)} className={flat}>
                <ChevronDownIcon className="size-4" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="z-[120]">
              <DropdownMenuItem onSelect={() => void open(null)}>{t(copy.local)}</DropdownMenuItem>
              {lanes === null || lanes.length === 0 ? null : <DropdownMenuSeparator />}
              {lanes?.map((l) => (
                <DropdownMenuItem key={l.id} disabled={!l.installed} onSelect={() => void open(l.id)}>
                  <span className="w-3 font-mono font-semibold">{l.id}</span>
                  <span className="flex-1">{l.name}</span>
                  {!l.installed && <span className="text-xs text-muted-foreground">{t(copy.notInstalled)}</span>}
                </DropdownMenuItem>
              ))}
              {lanes?.length === 0 && <DropdownMenuItem disabled>{t(copy.noLanes)}</DropdownMenuItem>}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <div className="ms-auto flex items-center ps-3">
          <Icon label={t(copy.close)} onClick={onClose}>
            <XIcon className="size-4" />
          </Icon>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 flex-col bg-white">
        {error && (
          <p className="px-3 pt-2 text-[13px] text-red-700" style={{ fontFamily: FONT }}>
            {error}
          </p>
        )}
        {/* Mounted for the life of the session: this view's xterm outlives a tab switch. */}
        {sessions.map((s) => (
          <TerminalView key={s.id} session={s} active={s.id === active} ended={t(copy.ended)} />
        ))}
      </div>

      {menu && (
        <div
          role="menu"
          className="fixed z-[110] min-w-32 rounded-md border bg-white p-1 text-[13px] shadow-md"
          style={{ left: menu.x, top: menu.y }}
        >
          <button
            role="menuitem"
            className="w-full rounded-sm px-2 py-1.5 text-start hover:bg-black/5"
            onClick={() => {
              setEditing(menu.id)
              setMenu(null)
            }}
          >
            {t(copy.rename)}
          </button>
        </div>
      )}
    </section>
  )
}

/** The Terminal page: a card per launcher. Pressing one opens its terminal full screen. */
export default function TerminalContent({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const list = useSessions(inApp === true)
  const [lanes, setLanes] = useState<TerminalLane[] | null>(null)
  const [full, setFull] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (inApp) terminalLanes().then(setLanes).catch(() => setLanes([]))
  }, [inApp])

  const enter = useCallback(() => {
    setFull(true)
    void terminalFullscreen(true).catch(() => {})
  }, [])
  const leave = useCallback(() => {
    setFull(false)
    void terminalFullscreen(false).catch(() => {})
  }, [])
  // Leaving the page leaves native full screen too; the sessions run on.
  useEffect(() => () => void terminalFullscreen(false).catch(() => {}), [])

  const start = async (lane: TerminalLane) => {
    setError(null)
    try {
      await terminalOpen(lane.id, lastGrid.cols, lastGrid.rows)
      enter()
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e))
    }
  }
  const reopen = (id: string) => {
    void terminalSetActive(id).catch(() => {})
    enter()
  }

  if (inApp === null) return null
  if (!inApp) return <OpenInMachine lang={lang} />

  const cards = [...(lanes ?? [])].sort((x, y) => {
    const i = (l: TerminalLane) => (CARD_ORDER.includes(l.id) ? CARD_ORDER.indexOf(l.id) : CARD_ORDER.length)
    return i(x) - i(y)
  })
  const sessions = list?.sessions ?? []
  return (
    <>
      <div
        className={cn(
          "flex h-full min-h-0 min-w-0 flex-col gap-6 overflow-y-auto ps-[26px] pe-6 pb-6 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:ps-0 lg:pe-16 lg:pb-16",
          SHELL_TOP,
        )}
      >
        <div className="flex max-w-md flex-col gap-2">
          {lanes === null && <p className="text-muted-foreground">{t(copy.loading)}</p>}
          {lanes?.length === 0 && <p className="text-muted-foreground">{t(copy.noLanes)}</p>}
          {cards.map((l) => (
            <button
              key={l.id}
              disabled={!l.installed}
              onClick={() => void start(l)}
              className="bg-background hover:bg-background/60 flex items-center gap-3 rounded-2xl p-4 text-start transition-colors disabled:opacity-50"
            >
              <Logo c={{ id: l.id, name: l.name, logo: logoOf(l.id), blurb: NO_BLURB }} />
              <span className="flex min-w-0 flex-1 items-center gap-2">
                <span className="truncate font-semibold">
                  <bdi>{l.name}</bdi>
                </span>
                {l.id === "f" && <span className="bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-xs">{t(copy.free)}</span>}
              </span>
              {l.installed ? (
                <span className="text-muted-foreground font-mono text-sm">{l.id}</span>
              ) : (
                <span className="text-muted-foreground text-sm">{t(copy.notInstalled)}</span>
              )}
            </button>
          ))}
          {error && <p className="text-destructive text-sm">{error}</p>}
        </div>

        {sessions.length > 0 && (
          <div className="flex max-w-md flex-col gap-1">
            <h2 className="text-muted-foreground px-1 text-sm">{t(copy.openSessions)}</h2>
            {sessions.map((s) => (
              <button
                key={s.id}
                onClick={() => reopen(s.id)}
                className="hover:bg-background/60 flex items-center gap-3 rounded-xl px-2 py-1.5 text-start transition-colors"
              >
                <Logo c={{ id: s.lane, name: s.name, logo: logoOf(s.lane), blurb: NO_BLURB }} size={28} />
                <span className="min-w-0 flex-1 truncate">
                  <bdi>{s.name}</bdi>
                </span>
                {!s.alive && <span className="text-muted-foreground text-xs">{t(copy.ended)}</span>}
              </button>
            ))}
          </div>
        )}
      </div>

      {full && (
        <div className="fixed inset-0 z-[100] bg-white">
          <TerminalTool lang={lang} onClose={leave} />
        </div>
      )}
    </>
  )
}
