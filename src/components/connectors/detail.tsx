import { ExternalLink } from "lucide-react"

import type { Copy } from "../chat/config"
import { Badges, kindLabel, Logo } from "./card"
import { copy } from "./config"
import { CATEGORIES, type Connector } from "./registry"

export function Detail({ c, installed, t }: { c: Connector; installed: boolean; t: (c: Copy) => string }) {
  const category = t(CATEGORIES.find((x) => x.id === c.category)!.label)
  return (
    <>
      <div className="flex flex-wrap items-center gap-4">
        <Logo c={c} size={64} />
        <div className="flex min-w-0 flex-col gap-0.5">
          <h2 dir="ltr" className="text-start text-xl font-semibold break-all">
            {c.name}
          </h2>
          <p className="text-muted-foreground text-sm">
            {kindLabel(c, t)} · {category}
            {installed ? ` · ${t(copy.installed)}` : ""}
          </p>
        </div>
      </div>
      <div className="bg-background flex flex-col gap-4 rounded-2xl p-4">
        <Badges c={c} installed={installed} t={t} />
        <p className="text-sm leading-relaxed">{t(c.blurb)}</p>
        {c.keywords.length > 0 && (
          <div className="flex flex-col gap-2">
            <p className="text-muted-foreground text-xs">{t(copy.keywords)}</p>
            <div className="flex flex-wrap gap-1.5">
              {c.keywords.map((k) => (
                <span key={k} dir="ltr" className="bg-muted rounded-full px-2.5 py-0.5 text-xs">
                  {k}
                </span>
              ))}
            </div>
          </div>
        )}
        {c.mcp && c.mcp.length > 0 && (
          <div className="flex flex-col gap-2">
            <p className="text-muted-foreground text-xs">{t(copy.mcpNames)}</p>
            <div className="flex flex-wrap gap-1.5">
              {c.mcp.map((m) => (
                <code key={m} dir="ltr" className="bg-muted rounded-md px-2 py-0.5 text-xs">
                  {m}
                </code>
              ))}
            </div>
          </div>
        )}
        <a
          href={c.url}
          target="_blank"
          rel="noreferrer"
          dir="ltr"
          className="text-foreground inline-flex w-fit items-center gap-1.5 text-sm underline underline-offset-4"
        >
          {t(copy.open)}
          <ExternalLink className="size-3.5" aria-hidden />
        </a>
      </div>
    </>
  )
}
