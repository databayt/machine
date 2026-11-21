"use client"

import { useEffect, useState } from "react"

import type { Locale } from "@/components/local/config"
import { PageShell } from "../shell/page"
import { OpenInMachine, useInMachine } from "../shell/content"
import { loadSettings } from "./actions"
import { DepartmentSection } from "./department"
import { SettingsForm } from "./form"
import { LanguageToggle } from "./language"
import type { Settings } from "./types"

/** Who you are and which department you work in. The agent, auto tasks and tools moved to Agents & tools. */
export default function SettingsContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const inApp = useInMachine()
  const [state, setState] = useState<Settings | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!inApp) return
    loadSettings()
      .then((settings) => {
        if (settings) setState(settings)
      })
      .catch((e) => setError(String(e)))
  }, [inApp])

  if (inApp === null) return null

  return (
    <PageShell title={isAr ? "الإعدادات" : "Settings"}>
      <LanguageToggle lang={lang} />
      {!inApp ? (
        <OpenInMachine lang={lang} />
      ) : (
        <>
          {error && <p className="text-destructive text-sm">{error}</p>}
          {state && <SettingsForm isAr={isAr} initial={state} />}
          <DepartmentSection lang={lang} />
        </>
      )}
    </PageShell>
  )
}
