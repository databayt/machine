"use client"

import { Search } from "lucide-react"
import { useSearchParams } from "next/navigation"
import { useEffect, useMemo, useState } from "react"

import { tr } from "@/components/chat/config"
import type { Locale } from "@/components/local/config"
import { cn } from "@/lib/utils"

import { mcpList } from "../config/actions"
import { useInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { Grid } from "./card"
import { copy } from "./config"
import { Detail } from "./detail"
import { CATEGORIES, CONNECTORS, isInstalled, KINDS, matches, type Category, type Kind } from "./registry"

const chip = "rounded-full px-3 py-1 text-sm transition-colors"

/** `/connectors/` → `?item=<id>`; read in the client (static export). */
export default function Connectors({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const id = useSearchParams().get("item")
  const [names, setNames] = useState<string[]>([])
  const [query, setQuery] = useState("")
  const [category, setCategory] = useState<Category | "all">("all")
  const [kind, setKind] = useState<Kind | "all">("all")

  useEffect(() => {
    if (!inApp) return
    mcpList()
      .then((v) => setNames(v.installed.map((s) => s.name)))
      .catch(() => setNames([]))
  }, [inApp])

  const have = (c: (typeof CONNECTORS)[number]) => isInstalled(c, names)
  const filtering = query.trim() !== "" || category !== "all" || kind !== "all"
  const results = useMemo(
    () => CONNECTORS.filter((c) => matches(c, query) && (category === "all" || c.category === category) && (kind === "all" || c.kind === kind)),
    [query, category, kind],
  )

  if (id) {
    const c = CONNECTORS.find((x) => x.id === id)
    return (
      <PageShell title={c?.name ?? t(copy.title)}>
        {c ? <Detail c={c} installed={have(c)} t={t} /> : <div className="bg-background text-muted-foreground w-fit rounded-2xl p-4 text-sm">{t(copy.notFound)}</div>}
      </PageShell>
    )
  }

  const pill = (on: boolean) => cn(chip, on ? "bg-foreground text-background" : "bg-muted text-foreground hover:bg-accent")
  const grid = (items: typeof CONNECTORS) => <Grid items={items} lang={lang} have={have} t={t} />

  return (
    <PageShell title={t(copy.title)} className="max-w-6xl">
      <div className="flex flex-col gap-4">
        <label className="bg-muted flex items-center gap-2 rounded-full px-4 py-2">
          <Search className="text-muted-foreground size-4 shrink-0" aria-hidden />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t(copy.search)}
            aria-label={t(copy.search)}
            className="placeholder:text-muted-foreground w-full bg-transparent text-sm outline-none"
          />
        </label>
        <div role="group" aria-label={t(copy.category)} className="flex flex-wrap gap-2">
          <button type="button" aria-pressed={category === "all"} onClick={() => setCategory("all")} className={pill(category === "all")}>
            {t(copy.all)}
          </button>
          {CATEGORIES.map((c) => (
            <button key={c.id} type="button" aria-pressed={category === c.id} onClick={() => setCategory(c.id)} className={pill(category === c.id)}>
              {t(c.label)}
            </button>
          ))}
        </div>
        <div role="group" aria-label={t(copy.type)} className="text-muted-foreground flex flex-wrap gap-x-4 gap-y-1 text-xs">
          {[{ id: "all" as const, label: copy.all }, ...KINDS].map((k) => (
            <button
              key={k.id}
              type="button"
              aria-pressed={kind === k.id}
              onClick={() => setKind(k.id)}
              className={cn("transition-colors hover:text-foreground", kind === k.id && "text-foreground font-medium")}
            >
              {t(k.label)}
            </button>
          ))}
        </div>
      </div>

      {filtering ? (
        results.length ? grid(results) : <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>
      ) : (
        <>
          <section className="flex flex-col gap-3">
            <h2 className="text-muted-foreground text-sm">{t(copy.top)}</h2>
            {grid(CONNECTORS.filter((c) => c.top))}
          </section>
          <section className="flex flex-col gap-8">
            <h2 className="text-muted-foreground text-sm">{t(copy.all)}</h2>
            {CATEGORIES.map((cat) => {
              const items = CONNECTORS.filter((c) => c.category === cat.id)
              return items.length ? (
                <div key={cat.id} className="flex flex-col gap-3">
                  <h3 className="font-semibold">{t(cat.label)}</h3>
                  {grid(items)}
                </div>
              ) : null
            })}
          </section>
        </>
      )}
    </PageShell>
  )
}
