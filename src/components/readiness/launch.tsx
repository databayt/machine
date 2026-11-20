"use client"

// On launch: show Readiness first until a check has ever reached 100%, and check for an app update
// (now and every 24 hours). Renders only a small link when an update is waiting.
import { useEffect } from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { isTauri } from "@tauri-apps/api/core"

import type { Locale } from "@/components/local/config"

import { useUpdateCheck } from "../update/content"
import { readinessState } from "./actions"

const SEEN = "machine.readiness.launch"

export function LaunchGate({ lang }: { lang: Locale }) {
  const router = useRouter()
  const { update } = useUpdateCheck()

  useEffect(() => {
    if (!isTauri() || sessionStorage.getItem(SEEN)) return
    sessionStorage.setItem(SEEN, "1")
    readinessState()
      .then((v) => {
        // No state file, or below 100%: this computer is not done. No catalog: nothing to show.
        if (v && v.catalogFound && (!v.hasState || v.percent < 100)) router.replace(`/${lang}/readiness/`)
      })
      .catch(() => null)
  }, [lang, router])

  if (!update) return null
  return (
    <Link
      href={`/${lang}/health/`}
      className="fixed bottom-4 end-4 z-50 rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background shadow-lg"
    >
      {lang === "ar" ? `تحديث ${update.version} جاهز` : `Update ${update.version} ready`}
    </Link>
  )
}
