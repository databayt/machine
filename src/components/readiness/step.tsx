"use client"

import { useState } from "react"
import Link from "next/link"

import type { Locale } from "@/components/local/config"

import ReposContent from "../repos/content"
import { OpenInMachine } from "../shell/content"
import { PageShell, Section } from "../shell/page"
import AgentsContent from "./agents"
import { KEYSTONE_INSTALLER, fixLabel, pick, wizard } from "./config"
import { LogPane, Mark, Messages, Quiet, button, quiet, word } from "./parts"
import { isPassing, useReadiness } from "./use-readiness"
import type { StepView } from "./types"

const mono = "font-mono text-sm break-words"

/** `/setup/?stage=<id>&step=<id>` — one step, with its install and uninstall. */
export default function StepScreen({ lang, stage, step }: { lang: Locale; stage: string; step: string }) {
  if (stage === "repos" && step === "all") return <ReposContent lang={lang} />
  if (stage === "agents" && step === "default") return <AgentsContent lang={lang} />
  return <StepDetail lang={lang} stage={stage} stepId={step} />
}

function StepDetail({ lang, stage, stepId }: { lang: Locale; stage: string; stepId: string }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const r = useReadiness(isAr)
  const [confirming, setConfirming] = useState(false)
  const { view, steps, removed } = r

  if (r.inApp === null) return null
  if (r.inApp === false)
    return (
      <PageShell title={stepId}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  const s = steps.find((x) => x.id === stepId)
  if (view && !s)
    return (
      <PageShell title={stepId}>
        <Quiet>{t(wizard.nothing)}</Quiet>
      </PageShell>
    )
  if (!s) return <PageShell title={stepId}>{null}</PageShell>

  const isRemoved = removed.includes(s.id)
  const status = isRemoved ? "removed" : s.status
  const title = pick(s.title, isAr, s.id)
  const href = (id: string) => `/${lang}/setup/?stage=${encodeURIComponent(steps.find((x) => x.id === id)?.stage ?? stage)}&step=${encodeURIComponent(id)}`
  const titleOf = (id: string) => pick(steps.find((x) => x.id === id)?.title, isAr, id)
  const neededBy = steps.filter((x) => x.after.includes(s.id))
  const dependents = neededBy.filter((x) => isPassing(x.status))
  const needsFix = s.status === "fail" || s.status === "blocked" || s.status === "human"

  const installLabel = isRemoved ? t(wizard.install) : s.fix && needsFix ? t(fixLabel[s.fix.kind]) : null
  const updateLabel = !isRemoved && s.update && isPassing(s.status) ? t(wizard.update) : null
  // Only something that is on the computer can be removed from it.
  const canUninstall = !!s.uninstall && !isRemoved && (isPassing(s.status) || s.status === "update-available")
  const runs = s.fix?.cmd || s.fix?.url || s.fix?.message || null
  const everyone = (l: string[]) => l.length === 0 || l.includes("*")
  const applies = [
    !everyone(s.roles) ? `${t(wizard.roles)}: ${s.roles.join(", ")}` : "",
    !everyone(s.departments) ? `${t(wizard.departments)}: ${s.departments.join(", ")}` : "",
    !everyone(s.os) ? `${t(wizard.os)}: ${s.os.join(", ")}` : "",
  ].filter(Boolean)

  function remove() {
    setConfirming(false)
    r.uninstall(s!.id)
  }

  const stepLog = r.log.filter((l) => l.id === null || l.id === s.id).map((l) => l.text)

  return (
    <PageShell title={title}>
      <div className="rounded-2xl bg-background p-4">
        <div className="flex items-start gap-3">
          <Mark status={status} />
          <div className="min-w-0">
            <p className="font-medium">{title}</p>
            <p className="mt-1 text-sm text-muted-foreground">{isRemoved ? t(wizard.removedByYou) : word(status, isAr)}</p>
            {s.detail && !isRemoved && (
              <p className={`mt-2 whitespace-pre-wrap text-muted-foreground ${mono}`} dir="ltr">
                {s.detail}
              </p>
            )}
          </div>
        </div>

        {!confirming && (
          <div className="mt-4 flex flex-wrap gap-2">
            {installLabel && (
              <button className={button} disabled={r.busy} onClick={() => r.act(s)}>
                {installLabel}
              </button>
            )}
            {updateLabel && (
              <button className={quiet} disabled={r.busy} onClick={() => r.act({ ...s, status: "update-available" } as StepView)}>
                {updateLabel}
              </button>
            )}
            {canUninstall && (
              <button className={quiet} disabled={r.busy} onClick={() => setConfirming(true)}>
                {t(wizard.uninstall)}
              </button>
            )}
          </div>
        )}

        {confirming && s.uninstall && (
          <div className="mt-4 flex flex-col gap-3 rounded-xl bg-muted p-4">
            <p className="text-sm text-muted-foreground">{t(wizard.confirmRemove)}</p>
            <p className={mono} dir="ltr">
              {s.uninstall.cmd}
            </p>
            {dependents.length > 0 && (
              <div>
                <p className="text-sm text-muted-foreground">{t(wizard.dependents)}</p>
                <ul className="mt-1 list-disc ps-5 text-sm">
                  {dependents.map((d) => (
                    <li key={d.id}>{titleOf(d.id)}</li>
                  ))}
                </ul>
              </div>
            )}
            {s.uninstall.keystone && (
              <div className="text-sm text-destructive">
                <p>{t(wizard.keystone)}</p>
                <p className={`mt-1 ${mono}`} dir="ltr">
                  {KEYSTONE_INSTALLER}
                </p>
              </div>
            )}
            <div className="flex flex-wrap gap-2">
              <button className={button} disabled={r.busy} onClick={remove}>
                {t(wizard.remove)}
              </button>
              <button className={quiet} onClick={() => setConfirming(false)}>
                {t(wizard.cancel)}
              </button>
            </div>
          </div>
        )}
      </div>

      <Messages error={r.error} notice={r.notice} />
      {r.busy && <LogPane lines={stepLog} />}

      {runs && (
        <Section title={t(wizard.runs)}>
          <p className={`rounded-2xl bg-background p-4 ${mono}`} dir="ltr">
            {runs}
          </p>
        </Section>
      )}

      {s.after.length > 0 && (
        <Section title={t(wizard.waitsFor)}>
          <StepLinks ids={s.after} href={href} titleOf={titleOf} />
        </Section>
      )}

      {neededBy.length > 0 && (
        <Section title={t(wizard.neededBy)}>
          <StepLinks ids={neededBy.map((x) => x.id)} href={href} titleOf={titleOf} />
        </Section>
      )}

      {s.update && (
        <Section title={t(wizard.keptUpToDate)}>
          <div className="rounded-2xl bg-background p-4">
            <p className={mono} dir="ltr">
              {s.update.cmd}
            </p>
            <p className="mt-1 text-sm text-muted-foreground" dir="ltr">
              {s.update.cadence}
              {s.update.auto ? ` · ${t(wizard.auto)}` : ""}
            </p>
          </div>
        </Section>
      )}

      {applies.length > 0 && (
        <Section title={t(wizard.appliesTo)}>
          <div className="rounded-2xl bg-background p-4 text-sm" dir="ltr">
            {applies.map((a) => (
              <p key={a}>{a}</p>
            ))}
          </div>
        </Section>
      )}
    </PageShell>
  )
}

function StepLinks({ ids, href, titleOf }: { ids: string[]; href: (id: string) => string; titleOf: (id: string) => string }) {
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {ids.map((id) => (
        <li key={id} className="min-w-0">
          <Link
            href={href(id)}
            className="block truncate rounded-2xl bg-background px-4 py-3 text-sm font-medium outline-none transition-colors hover:bg-background/60 focus-visible:ring-2 focus-visible:ring-foreground/40"
          >
            {titleOf(id)}
          </Link>
        </li>
      ))}
    </ul>
  )
}
