"use client"

// shell — the one place that reads the address and the machine to build the navigation tree, shared by
// the sidebar (lg and up) and the menu footer (below lg).
import { useEffect, useMemo, useState } from "react"
import { usePathname, useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"
import { getProfile } from "../profile/actions"
import { readinessState } from "../readiness/actions"
import { webSeats } from "../web/actions"
import type { ReadinessView } from "../readiness/types"
import { useInMachine } from "./content"
import { buildTree, trailOf, type NavNode, type Trail } from "./nav"

export interface NavState {
  inApp: boolean | null
  engine: boolean
  view: ReadinessView | null
  /** The readiness percent, or null before a first run / outside the app. */
  readyPct: number | null
  tree: NavNode[]
  trail: Trail
}

export function useNav(lang: Locale): NavState {
  const inApp = useInMachine()
  const pathname = usePathname() ?? ""
  const params = useSearchParams()
  const [engine, setEngine] = useState(false)
  const [view, setView] = useState<ReadinessView | null>(null)
  const [seats, setSeats] = useState<{ id: string; name: string }[]>([])

  useEffect(() => {
    if (!inApp) return
    getProfile()
      .then((p) => setEngine(!p || p.access.engine))
      .catch(() => setEngine(false))
    const reload = () =>
      readinessState()
        .then(setView)
        .catch(() => setView(null))
    reload()
    webSeats()
      .then((l) => setSeats(l.seats.map((x) => ({ id: x.id, name: x.name }))))
      .catch(() => setSeats([]))
    // A run, fix, heal or uninstall finished: the sidebar's counts follow without a reload.
    window.addEventListener("kun:readiness", reload)
    // The doctor that runs by itself at launch or after an update: its verdict arrives as a Tauri event.
    let off: (() => void) | undefined
    let live = true
    import("@tauri-apps/api/event")
      .then((m) => m.listen("readiness:gate", reload))
      .then((f) => (live ? (off = f) : f()))
      .catch(() => {})
    return () => {
      live = false
      off?.()
      window.removeEventListener("kun:readiness", reload)
    }
  }, [inApp])

  const tree = useMemo(
    () => buildTree({ isAr: lang === "ar", inApp: inApp === true, engine, stages: view?.stages ?? [], steps: view?.steps ?? [], seats }),
    [lang, inApp, engine, view, seats],
  )
  const trail = trailOf(pathname, params)
  const readyPct = view && view.hasState ? Math.round(view.percent) : null
  return { inApp, engine, view, readyPct, tree, trail }
}
