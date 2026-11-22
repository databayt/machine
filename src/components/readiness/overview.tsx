"use client"

import type { Locale } from "@/components/local/config"

import { CardGrid, NavCard } from "../shell/cards"
import { OpenInMachine } from "../shell/content"
import { appSteps, APP_STAGES, isInstalledApp } from "../apps/registry"
import { copy as appsCopy } from "../apps/config"
import { stageCounts } from "../shell/nav"
import { PageShell, Section } from "../shell/page"
import Link from "next/link"

import { cn } from "@/lib/utils"
import { copy, doctorCopy, fill, pick, wizard } from "./config"
import { Messages, button, quiet } from "./parts"
import type { GateView, StepView } from "./types"
import { needsWork, stageList, useReadiness } from "./use-readiness"

/** `/setup/` — one status card, then the stages. */
export default function Overview({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const r = useReadiness(isAr)
  const { view, steps, removed } = r

  if (r.inApp === null) return null
  if (r.inApp === false)
    return (
      <PageShell title={t(wizard.installAll)}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  const pct = Math.round(r.percent ?? 0)
  const failing = steps.filter((s) => !removed.includes(s.id) && (s.status === "fail" || s.status === "blocked" || s.status === "human"))
  const counts = stageCounts(steps)
  const apps = appSteps(steps, removed)
  const line = !view
    ? ""
    : !view.hasState
      ? t(wizard.neverRun)
      : failing.length === 0
        ? t(wizard.ready)
        : failing.length === 1
          ? t(wizard.needWorkOne)
          : fill(wizard.needWork, isAr, failing.length)

  return (
    <PageShell title={t(wizard.installAll)}>
      {view && !view.catalogFound ? (
        <p className="rounded-2xl bg-background p-4 text-sm text-muted-foreground">{t(copy.noCatalog)}</p>
      ) : (
        <>
          <div className="rounded-2xl bg-background p-4">
            <div className="flex items-end justify-between gap-3">
              <span className="text-5xl font-semibold tabular-nums" dir="ltr">
                {view?.hasState === false ? "–" : `${pct}%`}
              </span>
              {r.progress && (
                <span className="truncate text-sm text-muted-foreground" dir="ltr">
                  {r.progress}
                </span>
              )}
            </div>
            <div
              className="mt-3 h-2 overflow-hidden rounded-full bg-muted"
              role="progressbar"
              aria-valuenow={pct}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div className="h-full rounded-full bg-foreground transition-all" style={{ width: `${pct}%` }} />
            </div>
            <p className="mt-3 text-sm text-muted-foreground">{line}</p>
            <div className="mt-4 flex flex-wrap gap-2">
              <button className={button} disabled={r.busy} onClick={() => r.run("complete")}>
                {t(wizard.installAll)}
              </button>
              <button className={quiet} disabled={r.busy} onClick={() => r.run("check")}>
                {t(wizard.check)}
              </button>
              <button className={quiet} disabled={r.busy} onClick={() => r.run("maintain")}>
                {t(wizard.update)}
              </button>
              <button
                className={quiet}
                disabled={r.busy || failing.length === 0}
                onClick={() => r.heal(failing.map((s) => s.id))}
              >
                {t(wizard.heal)}
              </button>
              {(r.running === "check" || r.running === "complete" || r.running === "maintain") && (
                <button className={quiet} onClick={() => r.cancel()}>
                  {t(wizard.cancel)}
                </button>
              )}
            </div>
          </div>

          {view?.gate && (
            <DoctorCard lang={lang} gate={view.gate} steps={steps} busy={r.busy} running={r.running} onRun={r.doctor} onFix={r.doctorFix} />
          )}

          <Messages error={r.error} notice={r.notice} />

          <CardGrid>
            <NavCard
              href={`/${lang}/apps/`}
              name={t(appsCopy.title)}
              hint={t(appsCopy.hint)}
              state={`${apps.filter((x) => isInstalledApp(x, removed)).length}/${apps.length}`}
            />
            {stageList(view).filter((s) => !APP_STAGES.includes(s.id)).map((s) => {
              const c = counts[s.id] ?? { done: 0, total: 0 }
              const inStage = steps.filter((x) => x.stage === s.id && !removed.includes(x.id))
              const human = inStage.filter((x) => x.status === "human").length
              const toInstall = inStage.filter((x) => x.status === "fail" || x.status === "blocked").length
              const unchecked = inStage.length > 0 && inStage.every((x) => x.status === "pending")
              const hint = unchecked
                ? t(wizard.hintNotChecked)
                : [human ? fill(wizard.hintNeedYou, isAr, human) : "", toInstall ? fill(wizard.hintToInstall, isAr, toInstall) : ""]
                    .filter(Boolean)
                    .join(" · ") || (inStage.some((x) => needsWork(x.status)) ? "" : t(wizard.hintDone))
              return (
                <NavCard
                  key={s.id}
                  href={`/${lang}/setup/?stage=${encodeURIComponent(s.id)}`}
                  name={pick(s.title ?? undefined, isAr, s.id)}
                  hint={hint}
                  state={`${c.done}/${c.total}`}
                />
              )
            })}
          </CardGrid>

          {r.agentText && (
            <Section title={t(wizard.agentOut)}>
              <pre className="max-h-80 overflow-auto whitespace-pre-wrap rounded-2xl bg-background p-4 font-mono text-xs" dir="ltr">
                {r.agentText}
              </pre>
            </Section>
          )}
        </>
      )}
    </PageShell>
  )
}

/** Where a step lives: Apps owns its stages, Setup the rest. */
export function stepHref(lang: string, step: { id: string; stage: string } | undefined, id: string): string {
  if (step && APP_STAGES.includes(step.stage)) return `/${lang}/apps/?item=${encodeURIComponent(id)}`
  const stage = step?.stage
  return stage ? `/${lang}/setup/?stage=${encodeURIComponent(stage)}&step=${encodeURIComponent(id)}` : `/${lang}/setup/`
}

/** "Ready to contribute" or "Doctor: N steps left": the gate's failing steps, each linking to its page. */
function DoctorCard({
  lang,
  gate,
  steps,
  busy,
  running,
  onRun,
  onFix,
}: {
  lang: Locale
  gate: GateView
  steps: StepView[]
  busy: boolean
  running: string | null
  onRun: () => void
  onFix: () => void
}) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const n = gate.failing.length
  const never = gate.at === ""
  const title = gate.pass ? t(doctorCopy.ready) : never ? t(doctorCopy.notRun) : n === 1 ? t(doctorCopy.leftOne) : fill(doctorCopy.left, isAr, n)
  const hint = gate.pass ? t(doctorCopy.readyHint) : t(doctorCopy.leftHint)
  return (
    <div className="rounded-2xl bg-background p-4" data-gate={gate.pass ? "pass" : "fail"}>
      <p className={cn("text-xl font-semibold", gate.pass && "text-emerald-600")}>{title}</p>
      <p className="mt-1 text-sm text-muted-foreground">{hint}</p>
      {!gate.pass && n > 0 && (
        <ul className="mt-3 divide-y divide-border text-sm">
          {gate.failing.map((id) => {
            const step = steps.find((s) => s.id === id)
            return (
              <li key={id} className="flex items-center justify-between gap-3 py-2">
                <Link href={stepHref(lang, step, id)} className="min-w-0 truncate underline-offset-4 hover:underline">
                  {step ? pick(step.title, isAr, id) : id}
                </Link>
                {step?.status === "human" && <span className="shrink-0 text-xs text-amber-600">{t(doctorCopy.needsYou)}</span>}
              </li>
            )
          })}
        </ul>
      )}
      <div className="mt-4 flex flex-wrap gap-2">
        {!gate.pass && n > 0 && (
          <button className={button} disabled={busy} onClick={onFix}>
            {running === "doctorFix" ? t(doctorCopy.fixing) : t(doctorCopy.fix)}
          </button>
        )}
        <button className={quiet} disabled={busy} onClick={onRun}>
          {running === "doctor" ? t(doctorCopy.checking) : t(doctorCopy.run)}
        </button>
      </div>
    </div>
  )
}
