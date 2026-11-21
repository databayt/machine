"use client"

import Link from "next/link"
import { useSearchParams } from "next/navigation"
import { useMemo, useState } from "react"

import type { Locale } from "@/components/local/config"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"

import { copy, statusLabel, TYPES, typeLabel, type ItemType } from "./config"
import { PageShell, Section } from "../shell/page"
import { dirOfItem, typeOfDir } from "../shell/nav"
import { ItemActions } from "./item-actions"
import { useCanInstall, useCatalog } from "./use-catalog"

export default function CatalogContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const { items, error, reload } = useCatalog()
  const canInstallHere = useCanInstall()
  const dir = useSearchParams().get("dir") ?? ""
  // `?dir` preselects the type tab (the route remounts this on a new dir); `mcp` has no tab.
  const [type, setType] = useState<ItemType | "all">(() => {
    const preset = typeOfDir(dir)
    return (TYPES as readonly string[]).includes(preset ?? "") ? (preset as ItemType) : "pack"
  })
  const [query, setQuery] = useState("")

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    return (items ?? []).filter(
      (i) =>
        (type === "all" || i.id.startsWith(`${type}:`)) &&
        (!q || `${i.name} ${i.title} ${i.description} ${i.keywords.join(" ")}`.toLowerCase().includes(q))
    )
  }, [items, type, query])

  return (
    <PageShell title={t(copy.title)}>

      {!canInstallHere && (
        <div className="bg-background flex flex-wrap items-center justify-between gap-3 rounded-2xl p-4 text-sm">
          <span>{t(copy.getMachine)}</span>
          <Link href={`/${lang}/`} className="inline-flex h-9 items-center rounded-full bg-foreground px-4 text-sm font-medium text-background transition-colors hover:bg-foreground/90">
            {t(copy.getMachineButton)}
          </Link>
        </div>
      )}

      <Section className="gap-4">
        <Input
          value={query}
          className="h-12 rounded-full bg-background px-5 text-base shadow-none"
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t(copy.search)}
          aria-label={t(copy.search)}
        />
        <Tabs value={type} onValueChange={(v) => setType(v as ItemType | "all")}>
          <TabsList className="flex-wrap">
            {TYPES.map((k) => (
              <TabsTrigger key={k} value={k}>
                {t(typeLabel[k])}
              </TabsTrigger>
            ))}
            <TabsTrigger value="all">{t(copy.all)}</TabsTrigger>
          </TabsList>
        </Tabs>
      </Section>

      {error && <p className="text-destructive text-sm">{error}</p>}
      {!items && !error && <p className="text-muted-foreground">{t(copy.loading)}</p>}
      {items && shown.length === 0 && <p className="text-muted-foreground">{t(copy.empty)}</p>}

      <ul className="grid gap-3 sm:grid-cols-2">
        {shown.map((item) => (
          <li key={item.id} className="bg-background relative flex flex-col justify-between gap-3 rounded-2xl p-4 transition-colors focus-within:ring-2 focus-within:ring-foreground/40 hover:bg-background/60">
            <div>
              <div className="flex items-center justify-between gap-2">
                <Link
                  href={`/${lang}/catalog/?dir=${dir || dirOfItem(item.id) || ""}&item=${encodeURIComponent(item.id)}`}
                  className="font-medium no-underline outline-none after:absolute after:inset-0 after:rounded-2xl"
                >
                  {(isAr && item.title_ar) || item.title}
                </Link>
                {canInstallHere && (
                  <Badge variant={item.status === "installed" ? "default" : "secondary"}>
                    {t(statusLabel[item.status])}
                  </Badge>
                )}
              </div>
              <p className="text-muted-foreground mt-1 line-clamp-3 text-sm">{(isAr && item.description_ar) || item.description}</p>
            </div>
            <div className="relative z-10 self-start">
              <ItemActions item={item} isAr={isAr} onChanged={reload} />
            </div>
          </li>
        ))}
      </ul>
    </PageShell>
  )
}
