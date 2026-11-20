"use client"

import { useEffect, useState } from "react"

import { tools } from "./actions"
import type { ToolStatus } from "./types"

/** A small read-only list of the production tools on this machine, shown beside the agents. */
export function ToolsPanel({ isAr }: { isAr: boolean }) {
  const [list, setList] = useState<ToolStatus[] | null>(null)

  useEffect(() => {
    tools().then(setList).catch(() => setList([]))
  }, [])

  if (!list || list.length === 0) return null

  return (
    <section className="space-y-2">
      <h2 className="text-sm font-semibold">{isAr ? "أدوات الإنتاج" : "Production tools"}</h2>
      <ul className="bg-muted/70 divide-border divide-y rounded-2xl text-sm">
        {list.map((t) => (
          <li key={t.id} className="flex items-center justify-between gap-3 px-4 py-2.5">
            <span className="font-medium">{t.label}</span>
            <span className="text-muted-foreground min-w-0 truncate text-xs" dir="ltr">
              {t.installed ? (t.version ?? (isAr ? "مثبّت" : "installed")) : isAr ? "غير مثبّت" : "not installed"}
              {t.needs_key && ` · ${isAr ? "يحتاج مفتاحاً" : "needs key"}`}
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}
