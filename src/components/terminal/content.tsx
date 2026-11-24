"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { Maximize2Icon, Minimize2Icon, PlusIcon, XIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"

import { tr } from "../chat/config"
import { OpenInMachine, useInMachine } from "../shell/content"
import { SHELL_TOP } from "../shell/grid"
import {
  onTerminalData,
  onTerminalExit,
  terminalClose,
  terminalLanes,
  terminalOpen,
  terminalResize,
  terminalWrite,
  type TerminalLane,
} from "./actions"
import { copy } from "./config"

import "@xterm/xterm/css/xterm.css"

interface Tab {
  /** The id the terminal runs under (chosen here, so the first byte is never missed). */
  id: string
  lane: TerminalLane
}

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
}

/** Native full screen for the app window; a no-op where Tauri says no. */
async function nativeFullscreen(on: boolean) {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window")
    await getCurrentWindow().setFullscreen(on)
  } catch {}
}

let counter = 0
const newId = () => `w${Date.now().toString(36)}${(counter++).toString(36)}`

/** One xterm bound to one pty. Stays mounted (hidden) while another tab is in front. */
function TerminalView({ tab, active, ended }: { tab: Tab; active: boolean; ended: string }) {
  const host = useRef<HTMLDivElement>(null)
  const refit = useRef<() => void>(() => {})

  useEffect(() => {
    let dead = false
    const cleanups: (() => void)[] = []
    ;(async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit")])
      const el = host.current
      if (dead || !el) return
      const term = new Terminal({
        cursorBlink: true,
        fontSize: 13,
        fontFamily: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
        theme: LIGHT,
        allowProposedApi: false,
      })
      const fit = new FitAddon()
      term.loadAddon(fit)
      term.open(el)
      fit.fit()
      cleanups.push(() => term.dispose())

      const offData = await onTerminalData(tab.id, (b) => term.write(b))
      const offExit = await onTerminalExit(tab.id, (code) => {
        term.write(`\r\n\x1b[2m[${ended}${code === null ? "" : ` · ${code}`}]\x1b[0m\r\n`)
      })
      cleanups.push(offData, offExit)
      if (dead) return cleanups.forEach((c) => c())

      try {
        await terminalOpen(tab.lane.id, term.cols, term.rows, tab.id)
      } catch (e) {
        term.write(`\x1b[31m${String(e instanceof Error ? e.message : e)}\x1b[0m\r\n`)
        return
      }
      if (dead) {
        void terminalClose(tab.id).catch(() => {})
        return
      }
      const input = term.onData((d) => void terminalWrite(tab.id, d).catch(() => {}))
      const size = term.onResize(({ cols, rows }) => void terminalResize(tab.id, cols, rows).catch(() => {}))
      cleanups.push(() => input.dispose(), () => size.dispose())
      refit.current = () => {
        try {
          fit.fit()
        } catch {}
      }
      const ro = new ResizeObserver(() => refit.current())
      ro.observe(el)
      cleanups.push(() => ro.disconnect())
      term.focus()
    })()
    return () => {
      dead = true
      cleanups.forEach((c) => c())
    }
    // The tab is fixed for the life of this view.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (active) requestAnimationFrame(() => refit.current())
  }, [active])

  // The terminal is always left-to-right, whatever the page's direction.
  return <div ref={host} dir="ltr" className={cn("min-h-0 flex-1 overflow-hidden rounded-xl border bg-white p-3", !active && "hidden")} />
}

/** The Terminal screen: a lane picker, or the open terminals as tabs. Bare: no title, no links. */
export default function TerminalContent({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const [lanes, setLanes] = useState<TerminalLane[] | null>(null)
  const [tabs, setTabs] = useState<Tab[]>([])
  const [active, setActive] = useState<string | null>(null)
  const [picking, setPicking] = useState(false)
  const [full, setFull] = useState(false)

  useEffect(() => {
    if (inApp) terminalLanes().then(setLanes).catch(() => setLanes([]))
  }, [inApp])

  // Full screen = the terminals cover the whole window and the window goes native full screen.
  // Leaving the page, or closing the last tab, leaves it. Esc is not bound: claude, vim and the
  // agents all use it.
  const toggleFull = useCallback((on: boolean) => {
    setFull(on)
    void nativeFullscreen(on)
  }, [])
  useEffect(() => () => void nativeFullscreen(false), [])
  useEffect(() => {
    if (tabs.length === 0 && full) toggleFull(false)
  }, [tabs.length, full, toggleFull])

  const open = (lane: TerminalLane) => {
    const tab = { id: newId(), lane }
    setTabs((all) => [...all, tab])
    setActive(tab.id)
    setPicking(false)
  }

  const close = useCallback((id: string) => {
    void terminalClose(id).catch(() => {})
    setTabs((all) => {
      const rest = all.filter((x) => x.id !== id)
      setActive((cur) => (cur === id ? (rest.at(-1)?.id ?? null) : cur))
      return rest
    })
  }, [])

  if (inApp === null) return null
  if (!inApp) return <OpenInMachine lang={lang} />

  const showPicker = tabs.length === 0 || picking
  return (
    <section
      aria-label={t(copy.title)}
      className={cn(
        full
          ? "fixed inset-0 z-50 flex min-h-0 min-w-0 flex-col bg-white p-3 pt-[max(0.75rem,env(safe-area-inset-top))]"
          : cn(
              "flex h-full min-h-0 min-w-0 flex-col ps-[26px] pe-6 pb-6 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:ps-0 lg:pe-16 lg:pb-16",
              SHELL_TOP,
            ),
      )}
    >
      {tabs.length > 0 && (
        <div role="tablist" className="mb-3 flex shrink-0 items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              className={cn(
                "flex items-center rounded-full text-sm",
                tab.id === active && !picking ? "bg-foreground text-background" : "bg-background text-foreground hover:bg-background/70",
              )}
            >
              <button
                role="tab"
                aria-selected={tab.id === active && !picking}
                onClick={() => {
                  setActive(tab.id)
                  setPicking(false)
                }}
                className="flex items-center gap-2 py-1.5 ps-3 pe-1"
              >
                <span className="font-mono">{tab.lane.id}</span>
                <span className="max-w-40 truncate">{tab.lane.name}</span>
              </button>
              <button aria-label={t(copy.close)} onClick={() => close(tab.id)} className="rounded-full p-1.5 pe-2 opacity-70 hover:opacity-100">
                <XIcon className="size-3.5" />
              </button>
            </div>
          ))}
          <button
            aria-label={t(copy.add)}
            onClick={() => setPicking(true)}
            className="rounded-full p-2 text-muted-foreground hover:bg-background hover:text-foreground"
          >
            <PlusIcon className="size-4" />
          </button>
          <button
            aria-label={t(full ? copy.exitFull : copy.full)}
            title={t(full ? copy.exitFull : copy.full)}
            onClick={() => toggleFull(!full)}
            className="ms-auto rounded-full p-2 text-muted-foreground hover:bg-background hover:text-foreground"
          >
            {full ? <Minimize2Icon className="size-4" /> : <Maximize2Icon className="size-4" />}
          </button>
        </div>
      )}

      {showPicker && (
        <div className="flex max-w-md flex-col gap-2">
          {lanes === null && <p className="text-muted-foreground">{t(copy.loading)}</p>}
          {lanes?.length === 0 && <p className="text-muted-foreground">{t(copy.noLanes)}</p>}
          {lanes?.map((lane) => (
            <button
              key={lane.id}
              disabled={!lane.installed}
              onClick={() => open(lane)}
              className="flex items-center gap-4 rounded-2xl bg-background px-5 py-3.5 text-start transition-colors hover:bg-background/70 disabled:opacity-50"
            >
              <span className="w-4 font-mono text-lg font-semibold">{lane.id}</span>
              <span className="flex-1">{lane.name}</span>
              {!lane.installed && <span className="text-sm text-muted-foreground">{t(copy.notInstalled)}</span>}
            </button>
          ))}
        </div>
      )}

      {/* Mounted for the life of the tab; the picker only covers them. */}
      <div className={cn("flex min-h-0 flex-1 flex-col", showPicker && "hidden")}>
        {tabs.map((tab) => (
          <TerminalView key={tab.id} tab={tab} active={tab.id === active} ended={t(copy.ended)} />
        ))}
      </div>
    </section>
  )
}
