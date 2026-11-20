"use client"

import Link from "next/link"

import type { Locale } from "@/components/local/config"
import { Badge } from "@/components/ui/badge"

import { copy, statusLabel } from "./config"
import { ChevronLeftIcon } from "lucide-react"

import { PageShell, Section } from "../shell/page"
import { ItemActions } from "./item-actions"
import { useCanInstall, useCatalog } from "./use-catalog"

export default function CatalogDetail({ lang, type, name }: { lang: Locale; type: string; name: string }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const { items, error, reload } = useCatalog()
  const canInstallHere = useCanInstall()
  const item = items?.find((i) => i.id === `${type}:${name}`)

  return (
    <PageShell
      title={(item && ((isAr && item.title_ar) || item.title)) || t(copy.title)}
      description={item ? ((isAr && item.description_ar) || item.description) : undefined}
      actions={
        <Link
          href={`/${lang}/catalog/`}
          className="inline-flex items-center gap-1 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          <ChevronLeftIcon className="size-4 rtl:rotate-180" aria-hidden />
          {t(copy.back)}
        </Link>
      }
    >
      {error && <p className="text-destructive text-sm">{error}</p>}
      {!items && !error && <p className="text-muted-foreground">{t(copy.loading)}</p>}
      {items && !item && <p>{t(copy.notFound)}</p>}
      {item && (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <Badge variant="outline">{type}</Badge>
            {canInstallHere && <Badge>{t(statusLabel[item.status])}</Badge>}
            <ItemActions item={item} isAr={isAr} onChanged={reload} />
          </div>

          {item.dependencies.length > 0 && (
            <Section title={t(copy.dependencies)}>
              <ul className="flex flex-wrap gap-2">
                {item.dependencies.map((d) => (
                  <li key={d}>
                    <Link href={`/${lang}/catalog/item/?id=${encodeURIComponent(d)}`}>
                      <Badge variant="secondary">{d}</Badge>
                    </Link>
                  </li>
                ))}
              </ul>
            </Section>
          )}

          {item.keywords.length > 0 && (
            <Section title={t(copy.keywords)}>
              <div className="flex flex-wrap gap-2">
                {item.keywords.map((k) => (
                  <Badge key={k} variant="outline">
                    {k}
                  </Badge>
                ))}
              </div>
            </Section>
          )}

          <Section title={`${t(copy.files)} (${item.files.length})`}>
            <ul dir="ltr" className="space-y-1 font-mono text-xs text-muted-foreground">
              {item.files.map((f) => (
                <li key={f.target}>~/.claude/{f.target}</li>
              ))}
            </ul>
            <p className="text-xs text-muted-foreground">
              {t(copy.version)}: <span dir="ltr">{item.version.slice(0, 12)}</span>
            </p>
          </Section>
        </>
      )}
    </PageShell>
  )
}
