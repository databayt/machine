"use client"

import { useSearchParams } from "next/navigation"
import { useMemo, useState } from "react"

import { tr } from "@/components/chat/config"
import type { Locale } from "@/components/local/config"

import { Card, Grid, hoverAction } from "../plugins/card"
import { DeptPills, SearchField, useDeptFilter } from "../plugins/filters"
import { servesDept } from "../plugins/registry"
import { Messages } from "../readiness/parts"
import { pick } from "../readiness/config"
import type { StepView } from "../readiness/types"
import { useReadiness } from "../readiness/use-readiness"
import { OpenInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { AppAction, itemOf } from "./action"
import { copy } from "./config"
import { Detail } from "./detail"
import { appMeta, appSteps, isInstalledApp } from "./registry"

/** `/apps/` → `?item=<step id>`; the foundation, agents and tools steps as the plugins' cards. */
export default function Apps({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const r = useReadiness(isAr)
  const id = useSearchParams().get("item")
  const [query, setQuery] = useState("")
  const filter = useDeptFilter()
  const dept = filter.value
  const { view, steps, removed } = r

  const all = useMemo(() => appSteps(steps, removed), [steps, removed])
  const q = query.trim().toLowerCase()
  const results = useMemo(
    () =>
      all.filter((s) => {
        if (dept !== "all" && !servesDept(s.departments, dept)) return false
        if (!q) return true
        const i = itemOf(s, isAr)
        const m = appMeta(s.id)
        return [i.name, m?.blurb.en ?? "", m?.blurb.ar ?? "", s.id].some((x) => x.toLowerCase().includes(q))
      }),
    [all, dept, q, isAr],
  )

  if (r.inApp === null) return null
  if (r.inApp === false)
    return (
      <PageShell title={t(copy.title)}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  if (id) {
    const s = all.find((x) => x.id === id)
    return (
      <PageShell title={s ? itemOf(s, isAr).name : t(copy.title)}>
        {s ? (
          <>
            <Detail s={s} r={r} isAr={isAr} installed={isInstalledApp(s, removed)} t={t} />
            <Messages error={r.error} notice={r.notice} />
          </>
        ) : (
          <div className="bg-background text-muted-foreground w-fit rounded-2xl p-4 text-sm">{t(view ? copy.notFound : copy.empty)}</div>
        )}
      </PageShell>
    )
  }

  const grid = (list: StepView[]) => (
    <Grid>
      {list.map((s) => (
        <Card
          key={s.id}
          c={itemOf(s, isAr)}
          t={t}
          href={`/${lang}/apps/?item=${encodeURIComponent(s.id)}`}
          action={<AppAction s={s} r={r} installed={isInstalledApp(s, removed)} t={t} className={hoverAction} />}
        />
      ))}
    </Grid>
  )
  // With All, one section per stage (foundation, agents, tools), titled by the catalog.
  const stages = (view?.stages ?? []).map((st) => ({ st, items: results.filter((s) => s.stage === st.id) })).filter((x) => x.items.length)

  return (
    <PageShell title={t(copy.title)} className="max-w-6xl">
      <div className="flex flex-col gap-4">
        <DeptPills options={filter.options} value={dept} onChange={filter.set} t={t} />
        <SearchField value={query} onChange={setQuery} placeholder={t(copy.search)} />
      </div>
      <Messages error={r.error} notice={r.notice} />
      {view && !view.catalogFound ? (
        <p className="text-muted-foreground text-sm">{t(copy.noCatalog)}</p>
      ) : results.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>
      ) : dept === "all" && !q ? (
        <div className="flex flex-col gap-8">
          {stages.map(({ st, items }) => (
            <section key={st.id} className="flex flex-col gap-3">
              <h2 className="font-semibold">{pick(st.title ?? undefined, isAr, st.id)}</h2>
              {grid(items)}
            </section>
          ))}
        </div>
      ) : (
        grid(results)
      )}
    </PageShell>
  )
}
