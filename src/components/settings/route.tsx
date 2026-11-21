"use client"

import { useEffect, useState } from "react"
import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"
import { tr } from "../chat/config"
import { getProfile } from "../profile/actions"
import { useInMachine } from "../shell/content"
import { CardGrid, NavCard } from "../shell/cards"
import { settingsChildren } from "../shell/nav"
import { PageShell } from "../shell/page"
import SettingsContent from "./content"

/** `/settings/` — the four children as cards; `?view=general` — the general form (read in the client, static export). */
export default function SettingsRoute({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const view = useSearchParams().get("view")
  const [engine, setEngine] = useState(false)

  useEffect(() => {
    if (!inApp) return
    getProfile()
      .then((p) => setEngine(!p || p.access.engine))
      .catch(() => setEngine(false))
  }, [inApp])

  if (view === "general") return <SettingsContent lang={lang} />
  if (inApp === null) return null
  return (
    <PageShell title={t({ en: "Settings", ar: "الإعدادات" })}>
      <CardGrid>
        {settingsChildren(inApp, engine).map((c) => (
          <NavCard key={c.id} href={c.href(lang)} name={t(c.label)} hint={t(c.hint)} />
        ))}
      </CardGrid>
    </PageShell>
  )
}
