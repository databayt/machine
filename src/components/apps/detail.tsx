import type { Copy } from "../chat/config"
import { Logo } from "../plugins/card"
import { pick } from "../readiness/config"
import type { StepView } from "../readiness/types"
import type { Readiness } from "../readiness/use-readiness"
import { copy } from "./config"
import { AppAction, itemOf } from "./action"

/** One app: its mark, whether it is on the machine, what the check last said, and the one action. */
export function Detail({ s, r, isAr, installed, t }: { s: StepView; r: Readiness; isAr: boolean; installed: boolean; t: (c: Copy) => string }) {
  const item = itemOf(s, isAr)
  return (
    <>
      <div className="flex flex-wrap items-center gap-4">
        <Logo c={item} size={64} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <h2 className="text-xl font-semibold break-all">
            <bdi>{item.name}</bdi>
          </h2>
          <p className="text-muted-foreground text-sm">{t(installed ? copy.installed : copy.notInstalled)}</p>
        </div>
        <AppAction s={s} r={r} installed={installed} t={t} />
      </div>
      <div className="bg-background flex flex-col gap-3 rounded-2xl p-4">
        {item.blurb.en && <p className="text-sm leading-relaxed">{t(item.blurb)}</p>}
        <p className="text-muted-foreground text-xs">{pick(s.title, isAr, s.id)}</p>
        {s.detail && (
          <p dir="ltr" className="text-muted-foreground text-xs">
            {s.detail.split("\n")[0]}
          </p>
        )}
      </div>
    </>
  )
}
