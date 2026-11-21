"use client"

// shell — the one place that reads the address and the machine to build the navigation tree, shared by
// the sidebar (lg and up) and the menu footer (below lg).
import { useEffect, useMemo, useState } from "react"
import { usePathname, useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"
import { getProfile } from "../profile/actions"
import { readinessState } from "../readiness/actions"
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

  useEffect(() => {
    if (!inApp) return
    getProfile()
      .then((p) => setEngine(!p || p.access.engine))
      .catch(() => setEngine(false))
    readinessState()
      .then(setView)
      .catch(() => setView(null))
  }, [inApp])

  const tree = useMemo(
    () => buildTree({ isAr: lang === "ar", inApp: inApp === true, engine, stages: view?.stages ?? [], steps: view?.steps ?? [] }),
    [lang, inApp, engine, view],
  )
  const trail = trailOf(pathname, params)
  const readyPct = view && view.hasState ? Math.round(view.percent) : null
  return { inApp, engine, view, readyPct, tree, trail }
}
