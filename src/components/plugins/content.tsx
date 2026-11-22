"use client"

import { useSearchParams } from "next/navigation"
import { useCallback, useEffect, useMemo, useState } from "react"

import { tr } from "@/components/chat/config"
import type { Locale } from "@/components/local/config"

import { mcpAdd, mcpList, mcpRemove } from "../config/actions"
import { useInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { Grid, PluginCard, type Actions } from "./card"
import { copy } from "./config"
import { Detail } from "./detail"
import { DeptPills, SearchField, useDeptFilter } from "./filters"
import { CONNECTORS, DEPARTMENTS, isInstalled, matches, servesDept, type Connector } from "./registry"

/** `/plugins/` → `?item=<id>`; read in the client (static export). */
export default function Plugins({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const id = useSearchParams().get("item")
  const [names, setNames] = useState<string[]>([])
  const [catalog, setCatalog] = useState<string[]>([])
  const [query, setQuery] = useState("")
  const filter = useDeptFilter()
  const dept = filter.value

  const load = useCallback(async () => {
    try {
      const v = await mcpList()
      setNames(v.installed.map((s) => s.name))
      setCatalog(v.available.filter((s) => !s.skipped).map((s) => s.name))
    } catch {
      setNames([])
      setCatalog([])
    }
  }, [])

  useEffect(() => {
    if (inApp) void load()
  }, [inApp, load])

  /** Local server names only: claude.ai connectors and plugin servers are managed where they were added. */
  const local = (n: string) => !n.startsWith("claude.ai ") && !n.startsWith("plugin:")
  const actions: Actions = {
    installed: (c) => isInstalled(c, names),
    canAdd: (c) => inApp === true && (c.mcp ?? []).some((n) => catalog.includes(n)),
    canRemove: (c) => inApp === true && (c.mcp ?? []).some((n) => local(n) && names.includes(n)),
    add: async (c: Connector) => {
      const n = (c.mcp ?? []).find((x) => catalog.includes(x))
      if (n) await mcpAdd(n)
      await load()
    },
    remove: async (c: Connector) => {
      for (const n of (c.mcp ?? []).filter((x) => local(x) && names.includes(x))) await mcpRemove(n)
      await load()
    },
  }
  const filtering = query.trim() !== ""
  const results = useMemo(
    () => CONNECTORS.filter((c) => matches(c, query) && (dept === "all" || servesDept(c.departments, dept))),
    [query, dept],
  )

  if (id) {
    const c = CONNECTORS.find((x) => x.id === id)
    return (
      <PageShell title={c?.name ?? t(copy.title)}>
        {c ? <Detail c={c} actions={actions} t={t} /> : <div className="bg-background text-muted-foreground w-fit rounded-2xl p-4 text-sm">{t(copy.notFound)}</div>}
      </PageShell>
    )
  }

  const grid = (items: Connector[]) => (
    <Grid>
      {items.map((c) => (
        <PluginCard key={c.id} c={c} lang={lang} actions={actions} t={t} />
      ))}
    </Grid>
  )
  // With All, each plugin sits once, under its first department; the ones for everyone come last.
  const sections = [
    ...DEPARTMENTS.map((d) => ({ id: d.name, title: d.title, items: CONNECTORS.filter((c) => c.departments.find((x) => x !== "*") === d.name) })),
    { id: "*", title: copy.everyone, items: CONNECTORS.filter((c) => !c.departments.some((x) => x !== "*")) },
  ]

  return (
    <PageShell title={t(copy.title)} className="max-w-6xl">
      <div className="flex flex-col gap-4">
        <DeptPills options={filter.options} value={dept} onChange={filter.set} t={t} />
        <SearchField value={query} onChange={setQuery} placeholder={t(copy.search)} />
      </div>

      {filtering ? (
        results.length ? grid(results) : <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>
      ) : dept === "all" ? (
        <div className="flex flex-col gap-8">
          {sections.map((sec) =>
            sec.items.length ? (
              <section key={sec.id} className="flex flex-col gap-3">
                <h2 className="font-semibold">{t(sec.title)}</h2>
                {grid(sec.items)}
              </section>
            ) : null,
          )}
        </div>
      ) : (
        <>
          {results.some((c) => c.top) && (
            <section className="flex flex-col gap-3">
              <h2 className="text-muted-foreground text-sm">{t(copy.top)}</h2>
              {grid(results.filter((c) => c.top))}
            </section>
          )}
          <section className="flex flex-col gap-3">
            <h2 className="text-muted-foreground text-sm">{t(copy.all)}</h2>
            {results.some((c) => !c.top) ? grid(results.filter((c) => !c.top)) : <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>}
          </section>
        </>
      )}
    </PageShell>
  )
}
