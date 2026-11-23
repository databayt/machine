import { ExternalLink } from "lucide-react"

import type { Copy } from "../chat/config"
import { Action, kindLabel, Logo, stagedLine, type Actions } from "./card"
import { copy } from "./config"
import { DEPARTMENTS, type Connector } from "./registry"

export function Detail({ c, actions, t }: { c: Connector; actions: Actions; t: (c: Copy) => string }) {
  const depts = c.departments.includes("*")
    ? t(copy.everyone)
    : c.departments.map((d) => { const x = DEPARTMENTS.find((y) => y.name === d); return x ? t(x.title) : d }).join(" · ")
  return (
    <>
      <div className="flex flex-wrap items-center gap-4">
        <Logo c={c} size={64} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <h2 className="text-xl font-semibold break-all">
            <bdi>{c.name}</bdi>
          </h2>
          <p className="text-muted-foreground text-sm">
            {kindLabel(c, t)} · {depts}
          </p>
        </div>
        {!c.staged && <Action c={c} actions={actions} t={t} />}
      </div>
      <div className="bg-background flex flex-col gap-4 rounded-2xl p-4">
        <p className="text-sm leading-relaxed">{t(c.blurb)}</p>
        {c.staged && <p className="text-muted-foreground text-sm">{stagedLine(c, t)}</p>}
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
