"use client"

import Link from "next/link"
import { useMemo, useState } from "react"

import type { Locale } from "@/components/local/config"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"

import { copy, statusLabel, TYPES, typeLabel, type ItemType } from "./config"
import { PageShell, Section } from "../shell/page"
import { ItemActions } from "./item-actions"
import { useCanInstall, useCatalog } from "./use-catalog"

export default function CatalogContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const { items, error, reload } = useCatalog()
  const canInstallHere = useCanInstall()
  const [type, setType] = useState<ItemType | "all">("pack")
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
    <PageShell title={t(copy.title)} description={t(copy.subtitle)}>

      {!canInstallHere && (
        <div className="bg-background flex flex-wrap items-center justify-between gap-3 rounded-2xl p-4 text-sm">
          <span>{t(copy.getMachine)}</span>
          <Link href={`/${lang}/`} className="text-primary font-medium underline-offset-4 hover:underline">
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
          <li key={item.id} className="bg-background flex flex-col justify-between gap-3 rounded-2xl p-4">
            <div>
              <div className="flex items-center justify-between gap-2">
                <Link
                  href={`/${lang}/catalog/item/?id=${encodeURIComponent(item.id)}`}
                  className="font-medium hover:underline"
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
            <ItemActions item={item} isAr={isAr} onChanged={reload} />
          </li>
        ))}
      </ul>
    </PageShell>
  )
}
