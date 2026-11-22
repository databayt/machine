import Link from "next/link"

import type { Copy } from "../chat/config"
import { copy } from "./config"
import { KINDS, type Connector } from "./registry"

type T = (c: Copy) => string

/** A white tile (many logos are black, so it stays white in dark mode) with the logo or the first letter. */
export function Logo({ c, size = 40 }: { c: Connector; size?: number }) {
  return (
    <span
      className="flex shrink-0 items-center justify-center overflow-hidden rounded-xl bg-white ring-1 ring-black/10"
      style={{ width: size, height: size }}
    >
      {c.logo ? (
        // eslint-disable-next-line @next/next/no-img-element -- static export, no image optimizer
        <img src={c.logo} alt="" width={size * 0.6} height={size * 0.6} className="object-contain" style={{ width: size * 0.6, height: size * 0.6 }} />
      ) : (
        <span dir="ltr" className="font-semibold text-neutral-800" style={{ fontSize: size * 0.45 }}>
          {c.name.charAt(0).toUpperCase()}
        </span>
      )}
    </span>
  )
}

export const kindLabel = (c: Connector, t: T) => t(KINDS.find((k) => k.id === c.kind)!.label)

export function Badges({ c, installed, t }: { c: Connector; installed: boolean; t: T }) {
  return (
    <p className="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs">
      <span>{kindLabel(c, t)}</span>
      {c.verified && (
        <>
          <span aria-hidden>·</span>
          <span>{t(copy.verified)}</span>
        </>
      )}
      {installed && (
        <>
          <span aria-hidden>·</span>
          <span className="inline-flex items-center gap-1">
            <span aria-hidden className="size-1.5 rounded-full bg-green-500" />
            {t(copy.installed)}
          </span>
        </>
      )}
    </p>
  )
}

export function Card({ c, lang, installed, t }: { c: Connector; lang: string; installed: boolean; t: T }) {
  return (
    <Link
      href={`/${lang}/connectors/?item=${encodeURIComponent(c.id)}`}
      className="bg-background hover:bg-accent flex min-w-0 items-start gap-3 rounded-2xl p-4 transition-colors"
    >
      <Logo c={c} />
      <span className="flex min-w-0 flex-col gap-1">
        <span className="truncate font-semibold">
          <bdi>{c.name}</bdi>
        </span>
        <Badges c={c} installed={installed} t={t} />
        <span className="text-muted-foreground line-clamp-2 text-sm">{t(c.blurb)}</span>
      </span>
    </Link>
  )
}

export function Grid({ items, lang, have, t }: { items: Connector[]; lang: string; have: (c: Connector) => boolean; t: T }) {
  return (
    <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
      {items.map((c) => (
        <Card key={c.id} c={c} lang={lang} installed={have(c)} t={t} />
      ))}
    </div>
  )
}
