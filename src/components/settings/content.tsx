"use client"

import { useEffect, useState } from "react"
import Link from "next/link"

import type { Locale } from "@/components/local/config"
import { departments as listDepartments } from "../pack/actions"
import { PageShell, Section } from "../shell/page"
import { OpenInMachine, useInMachine } from "../shell/content"
import { loadSettings } from "./actions"
import { SettingsForm } from "./form"
import { LanguageToggle } from "./language"
import type { Settings } from "./types"

/** Who you are and which department you work in. The agent, auto tasks and tools moved to Agents & tools. */
export default function SettingsContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const inApp = useInMachine()
  const [state, setState] = useState<{ settings: Settings; departments: string[] } | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!inApp) return
    Promise.all([loadSettings(), listDepartments()])
      .then(([settings, departments]) => {
        if (settings) setState({ settings, departments })
      })
      .catch((e) => setError(String(e)))
  }, [inApp])

  if (inApp === null) return null

  return (
    <PageShell
      title={isAr ? "الإعدادات" : "Settings"}
      description={inApp ? (isAr ? "من أنت، وفي أي قسم تعمل." : "Who you are and which department you work in.") : undefined}
    >
      <LanguageToggle lang={lang} />
      {!inApp ? (
        <OpenInMachine lang={lang} />
      ) : (
        <>
          {error && <p className="text-destructive text-sm">{error}</p>}
          {state && <SettingsForm isAr={isAr} initial={state.settings} departments={state.departments} />}
          <p className="text-muted-foreground text-sm">
            <Link href={`/${lang}/agents/`} className="underline underline-offset-4">{isAr ? "الوكيل الافتراضي والأدوات" : "Default agent, auto tasks and tools"}</Link>
          </p>
        </>
      )}
    </PageShell>
  )
}
