"use client"

import { useSearchParams } from "next/navigation"
import { useCallback, useEffect, useMemo, useState } from "react"

import { tr } from "@/components/chat/config"
import type { Locale } from "@/components/local/config"

import { AppAction, itemOf } from "../apps/action"
import { copy as appsCopy } from "../apps/config"
import { Detail as AppDetail } from "../apps/detail"
import { appSteps, isInstalledApp } from "../apps/registry"
import { mcpAdd, mcpList, mcpRemove } from "../config/actions"
import { Messages } from "../readiness/parts"
import { useReadiness } from "../readiness/use-readiness"
import { useInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { Card, Grid, hoverAction, PluginCard, type Actions } from "./card"
import { copy } from "./config"
import { Detail } from "./detail"
import { DeptFilter, useDeptFilter } from "./filters"
import { CONNECTORS, isInstalled as isInstalledPlugin, servesDept, type Connector } from "./registry"

/**
 * `/plugins/` → `?item=<id>`; read in the client (static export). One page for what the agents use:
 * the plugins (connectors, plugins, MCP servers) and the apps (the programs on this computer, from
 * the readiness catalog). An id names a plugin first, then an app.
 */
export default function Plugins({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const inApp = useInMachine()
  const id = useSearchParams().get("item")
  const [names, setNames] = useState<string[]>([])
  const [catalog, setCatalog] = useState<string[]>([])
  const filter = useDeptFilter()
  const dept = filter.value
  const r = useReadiness(isAr)
  const { view, steps, removed } = r

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
    installed: (c) => isInstalledPlugin(c, names),
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

  const plugins = useMemo(() => CONNECTORS.filter((c) => dept === "all" || servesDept(c.departments, dept)), [dept])
  const apps = useMemo(() => appSteps(steps, removed).filter((s) => dept === "all" || servesDept(s.departments, dept)), [steps, removed, dept])

  if (id) {
    const c = CONNECTORS.find((x) => x.id === id)
    if (c)
      return (
        <PageShell title={c.name}>
          <Detail c={c} actions={actions} t={t} lang={lang} />
        </PageShell>
      )
    const s = appSteps(steps, removed).find((x) => x.id === id)
    if (s)
      return (
        <PageShell title={itemOf(s, isAr).name}>
          <AppDetail s={s} r={r} isAr={isAr} installed={isInstalledApp(s, removed)} t={t} />
          <Messages error={r.error} notice={r.notice} />
        </PageShell>
      )
    // The apps load after the page: until they do, an id that is no plugin may still be an app.
    const waiting = inApp === null || (inApp && !view)
    return (
      <PageShell title={t(copy.title)}>
        {!waiting && <div className="bg-background text-muted-foreground w-fit rounded-2xl p-4 text-sm">{t(copy.notFound)}</div>}
      </PageShell>
    )
  }

  const pluginGrid = (items: Connector[]) => (
    <Grid>
      {items.map((c) => (
        <PluginCard key={c.id} c={c} lang={lang} actions={actions} t={t} />
      ))}
    </Grid>
  )
  const appGrid = (
    <Grid>
      {apps.map((s) => (
        <Card
          key={s.id}
          c={itemOf(s, isAr)}
          t={t}
          href={`/${lang}/plugins/?item=${encodeURIComponent(s.id)}`}
          action={<AppAction s={s} r={r} installed={isInstalledApp(s, removed)} t={t} className={hoverAction} />}
        />
      ))}
    </Grid>
  )

  // Top (for a chosen department), then Plugins, then Apps; with All there is no Top. Each plugin sits once.
  const top = dept === "all" ? [] : plugins.filter((c) => c.top)
  const rest = plugins.filter((c) => !top.includes(c))
  const section = (key: string, title: string, body: React.ReactNode, muted = false) => (
    <section key={key} className="flex flex-col gap-3">
      <h2 className={muted ? "text-muted-foreground text-sm" : "font-semibold"}>{title}</h2>
      {body}
    </section>
  )

  return (
    <PageShell title={t(copy.title)} className="max-w-6xl">
      <div className="flex">
        <DeptFilter lang={lang} options={filter.options} value={dept} onChange={filter.set} />
      </div>
      <Messages error={r.error} notice={r.notice} />
      {plugins.length + apps.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>
      ) : (
        <div className="flex flex-col gap-8">
          {top.length > 0 && section("top", t(copy.top), pluginGrid(top))}
          {rest.length > 0 && section("plugins", t(copy.title), pluginGrid(rest))}
          {apps.length > 0 && section("apps", t(appsCopy.title), appGrid)}
        </div>
      )}
    </PageShell>
  )
}
