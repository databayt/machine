"use client"

import { useEffect, useRef } from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"

import type { Locale } from "@/components/local/config"

import { CardGrid, NavCard } from "../shell/cards"
import { OpenInMachine } from "../shell/content"
import { APP_STAGES } from "../apps/registry"
import { stageCounts } from "../shell/nav"
import { PageShell } from "../shell/page"
import { fixLabel, pick, wizard } from "./config"
import { LogPane, Mark, Messages, Quiet, button, quiet, word } from "./parts"
import { stageList, stageNeedsWork, useReadiness } from "./use-readiness"
import type { StepView } from "./types"

/** The label of the one action a step's card offers, or null when it needs nothing. */
export function actionLabel(step: StepView, removed: boolean, isAr: boolean): string | null {
  if (removed) return null
  const l = (t: { en: string; ar: string }) => (isAr ? t.ar : t.en)
  if (step.status === "update-available") return l(wizard.update)
  if ((step.status === "fail" || step.status === "blocked" || step.status === "human") && step.fix) return l(fixLabel[step.fix.kind])
  return null
}

/** `/setup/?stage=<id>` — the stage's steps as cards. */
export default function StageScreen({ lang, stage }: { lang: Locale; stage: string }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const router = useRouter()
  const r = useReadiness(isAr)
  const { view, steps, removed } = r
  const installing = useRef(false)

  // After "Install this stage" finishes with nothing left here, move on to the next stage that needs work.
  useEffect(() => {
    if (r.running !== null || !installing.current || !view) return
    installing.current = false
    if (stageNeedsWork(steps, removed, stage)) return
    const order = stageList(view).map((s) => s.id).filter((id) => !APP_STAGES.includes(id) || id === stage)
    const at = order.indexOf(stage)
    const next = order.slice(at + 1).concat(order.slice(0, at)).find((id) => stageNeedsWork(steps, removed, id))
    router.replace(next ? `/${lang}/setup/?stage=${encodeURIComponent(next)}` : `/${lang}/setup/`)
  }, [r.running, view, steps, removed, stage, lang, router])

  if (r.inApp === null) return null
  if (r.inApp === false)
    return (
      <PageShell title={stage}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  const info = stageList(view).find((s) => s.id === stage)
  const title = pick(info?.title ?? undefined, isAr, stage)
  if (view && !info)
    return (
      <PageShell title={stage}>
        <Quiet>{t(wizard.nothing)}</Quiet>
      </PageShell>
    )

  const inStage = steps.filter((s) => s.stage === stage)
  const ids = inStage.map((s) => s.id)
  const c = stageCounts(steps)[stage] ?? { done: 0, total: 0 }
  const installable = inStage.filter(
    (s) => !removed.includes(s.id) && (s.status === "fail" || s.status === "blocked") && !s.human && s.fix?.kind === "auto",
  )
  const href = (step: string) => `/${lang}/setup/?stage=${encodeURIComponent(stage)}&step=${encodeURIComponent(step)}`

  return (
    <PageShell title={title}>
      <div className="rounded-2xl bg-background p-4">
        <div className="flex items-end justify-between gap-3">
          <span className="text-4xl font-semibold tabular-nums" dir="ltr">
            {c.done}/{c.total}
          </span>
          {r.progress && (
            <span className="truncate text-sm text-muted-foreground" dir="ltr">
              {r.progress}
            </span>
          )}
        </div>
        {stage === "accounts" && (
          <p className="mt-2 text-sm text-muted-foreground">{t(wizard.requiredSigned).replace("{n}", String(c.total)).replace("{m}", String(c.done))}</p>
        )}
        <div className="mt-4 flex flex-wrap gap-2">
          <button
            className={button}
            disabled={r.busy || installable.length === 0}
            onClick={() => {
              installing.current = true
              r.run("complete", installable.map((s) => s.id))
            }}
          >
            {t(wizard.installStage)}
          </button>
          <button className={quiet} disabled={r.busy} onClick={() => r.run("check", ids)}>
            {t(wizard.check)}
          </button>
          {(r.running === "check" || r.running === "complete" || r.running === "maintain") && (
            <button className={quiet} onClick={() => r.cancel()}>
              {t(wizard.cancel)}
            </button>
          )}
        </div>
      </div>

      <Messages error={r.error} notice={r.notice} />
      {r.busy && <LogPane lines={r.log.map((l) => (l.id ? `${l.id}  ${l.text}` : l.text))} />}

      <CardGrid>
        {inStage.map((s) => {
          const isRemoved = removed.includes(s.id)
          const status = isRemoved ? "removed" : s.status
          const label = actionLabel(s, isRemoved, isAr)
          return (
            <li key={s.id} className="relative min-w-0 rounded-2xl bg-background p-4 transition-colors focus-within:ring-2 focus-within:ring-foreground/40 hover:bg-background/60">
              <Link href={href(s.id)} className="flex min-w-0 items-start gap-3 outline-none after:absolute after:inset-0 after:rounded-2xl">
                <Mark status={status} />
                <span className="min-w-0">
                  <span className="block truncate font-medium">{pick(s.title, isAr, s.id)}</span>
                  <span className="mt-1 block truncate text-sm text-muted-foreground">
                    {isRemoved ? t(wizard.removedByYou) : word(status, isAr)}
                    {s.detail && !isRemoved && (
                      <span dir="ltr"> · {s.detail.split("\n")[0]}</span>
                    )}
                  </span>
                </span>
              </Link>
              {label && (
                <div className="relative z-10 mt-3 flex">
                  <button className={quiet} disabled={r.busy} onClick={() => r.act(s)}>
                    {label}
                  </button>
                </div>
              )}
            </li>
          )
        })}
        {stage === "repos" && <NavCard href={href("all")} name={t(wizard.allRepos)} />}
        {stage === "agents" && <NavCard href={href("default")} name={t(wizard.defaultAgent)} />}
      </CardGrid>
    </PageShell>
  )
}
