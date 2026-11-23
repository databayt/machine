"use client"

import { tr } from "../chat/config"
import type { PlanView } from "../chat/block-pure"
import { copy } from "./config"
import { STAGES } from "./pure"

/** The stages a person signs off: the plan marks them. */
const GATED = new Set(["script", "visual", "approval", "publish"])

/**
 * The film plan a plan turn ends with: one row per stage (number, name, tool), a gate mark on the four gated stages and
 * the cost lines as one muted line. A card: no title, no links; Accept sits under it.
 */
export function PlanCard({ plan, isAr }: { plan: PlanView; isAr: boolean }) {
  const t = tr(isAr)
  return (
    <div dir="ltr" role="group" aria-label={t(copy.planLabel)} className="rounded-2xl bg-muted p-3 font-mono text-[13px] leading-6">
      <ul>
        {STAGES.map((name, i) => {
          const tool = plan.stages[name]?.tool
          return (
            <li key={name} className="flex items-baseline gap-[1ch]">
              <span className="w-[2ch] shrink-0 text-muted-foreground tabular-nums">{String(i + 1).padStart(2, "0")}</span>
              <span className="w-[11ch] shrink-0 text-foreground">{name}</span>
              <span className="min-w-0 flex-1 truncate text-muted-foreground">{typeof tool === "string" ? tool : ""}</span>
              {GATED.has(name) && (
                <span role="img" aria-label={t(copy.planGate)} title={t(copy.planGate)} className="shrink-0 text-[color:var(--cli-accent)]">
                  ◆
                </span>
              )}
            </li>
          )
        })}
      </ul>
      {plan.cost.length > 0 && (
        <p dir="auto" className="mt-2 break-words font-sans text-xs text-muted-foreground">
          {plan.cost.join(" · ")}
        </p>
      )}
    </div>
  )
}
