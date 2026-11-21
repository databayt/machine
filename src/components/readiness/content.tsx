"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type { Locale } from "@/components/local/config"

import { PageShell, Section } from "../shell/page"
import { OpenInMachine, useInMachine } from "../shell/content"
import { healWithAgent, readinessCancel, readinessFix, readinessRun, readinessState } from "./actions"
import { copy, fixLabel, pick, statusMark, statusWord, tone } from "./config"
import type { Mode, ReadinessView, RunEvent, StepStatus, StepView } from "./types"

const button =
  "inline-flex h-9 items-center rounded-full bg-foreground px-4 text-sm font-medium text-background transition-colors hover:bg-foreground/90 disabled:opacity-50"
const quiet = "inline-flex h-9 items-center rounded-full bg-muted px-4 text-sm font-medium transition-colors hover:bg-muted/70 disabled:opacity-50"
const link = "text-sm font-medium text-primary rounded-full px-2.5 py-1 transition-colors hover:bg-background disabled:opacity-50"

type Live = Record<string, { status: StepStatus; detail: string }>

/** The Readiness tab: stages as a vertical stepper, an overall bar, and the four actions. */
export default function ReadinessContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const inApp = useInMachine()
  const [view, setView] = useState<ReadinessView | null>(null)
  const [live, setLive] = useState<Live>({})
  const [percent, setPercent] = useState<number | null>(null)
  const [running, setRunning] = useState<Mode | "heal" | null>(null)
  const [progress, setProgress] = useState<string | null>(null)
  const [agentText, setAgentText] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [failing, setFailing] = useState<string[]>([])
  const mounted = useRef(true)

  const load = useCallback(async () => {
    const v = await readinessState()
    if (!mounted.current || !v) return
    setView(v)
    setPercent(v.percent)
    setFailing(v.failing)
  }, [])

  useEffect(() => {
    mounted.current = true
    if (inApp) load().catch((e) => setError(String(e)))
    return () => {
      mounted.current = false
    }
  }, [inApp, load])

  const onEvent = useCallback((e: RunEvent) => {
    if (e.type === "step") setLive((l) => ({ ...l, [e.id]: { status: e.status, detail: e.detail } }))
    else if (e.type === "progress") setProgress(e.label)
    else if (e.type === "start") setProgress(`${e.mode} · ${e.total}`)
    else if (e.type === "done") {
      setPercent(e.percent)
      setFailing(e.failing)
    } else if (e.type === "error") setError(e.message)
  }, [])

  async function go(mode: Mode) {
    setRunning(mode)
    setError(null)
    setNotice(null)
    setLive({})
    try {
      await readinessRun(mode, onEvent)
      await load()
    } catch (e) {
      setError(String(e))
    } finally {
      setRunning(null)
      setProgress(null)
    }
  }

  async function heal() {
    setRunning("heal")
    setError(null)
    setAgentText("")
    try {
      await healWithAgent(failing, (e) => {
        if (e.kind === "text") setAgentText((s) => s + e.text)
        else if (e.kind === "error") setError(e.message)
      })
      await load()
    } catch (e) {
      setError(String(e))
    } finally {
      setRunning(null)
    }
  }

  async function fix(step: StepView) {
    setError(null)
    setNotice(null)
    setLive((l) => ({ ...l, [step.id]: { status: "fixing", detail: "" } }))
    try {
      const out = await readinessFix(step.id, onEvent)
      if (out.message) setNotice(out.message)
      if (out.kind === "auto") await load().then(() => setLive((l) => omit(l, step.id)))
      else setLive((l) => omit(l, step.id))
    } catch (e) {
      setError(String(e))
      setLive((l) => omit(l, step.id))
    }
  }

  const stages = useMemo(() => {
    if (!view) return []
    const order = view.stages.map((s) => s.id)
    const extra = [...new Set(view.steps.map((s) => s.stage))].filter((s) => !order.includes(s))
    return [...view.stages, ...extra.map((id) => ({ id, title: null }))]
      .map((s) => ({ stage: s, steps: view.steps.filter((x) => x.stage === s.id) }))
      .filter((g) => g.steps.length > 0)
  }, [view])

  if (inApp === null) return null
  if (inApp === false)
    return (
      <PageShell title={t(copy.title)}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  const pct = Math.round(percent ?? 0)
  const busy = running !== null

  return (
    <PageShell title={t(copy.title)}>
      <Section>
      <div className="rounded-2xl bg-background p-4">
        <div className="mb-2 flex items-baseline justify-between text-sm">
          <span className="font-medium">{view?.hasState === false ? t(copy.neverRun) : `${pct}% ${t(copy.done)}`}</span>
          {progress && <span className="text-muted-foreground" dir="ltr">{progress}</span>}
        </div>
        <div className="h-2 overflow-hidden rounded-full bg-muted" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
          <div className="h-full rounded-full bg-foreground transition-all" style={{ width: `${pct}%` }} />
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <button className={button} disabled={busy} onClick={() => go("check")}>{t(copy.check)}</button>
          <button className={quiet} disabled={busy} onClick={() => go("complete")}>{t(copy.complete)}</button>
          <button className={quiet} disabled={busy} onClick={() => go("maintain")}>{t(copy.update)}</button>
          <button className={quiet} disabled={busy || failing.length === 0} onClick={heal}>{t(copy.heal)}</button>
          {busy && running !== "heal" && (
            <button className={quiet} onClick={() => readinessCancel()}>{t(copy.cancel)}</button>
          )}
        </div>
      </div>
      </Section>

      {error && <p className="text-sm text-destructive">{error}</p>}
      {notice && <p className="text-sm text-muted-foreground">{notice}</p>}
      {view && !view.catalogFound && <p className="text-sm text-muted-foreground">{t(copy.noCatalog)}</p>}

      {stages.map(({ stage, steps }) => (
        <Section key={stage.id} title={pick(stage.title ?? undefined, isAr, stage.id)}>
          <ul className="space-y-2">
            {steps.map((s) => (
              <Row key={s.id} step={s} live={live[s.id]} isAr={isAr} busy={busy} onFix={() => fix(s)} />
            ))}
          </ul>
        </Section>
      ))}

      {agentText && (
        <Section title={t(copy.agentOut)}>
          <pre className="max-h-80 overflow-auto whitespace-pre-wrap rounded-2xl bg-background p-4 text-xs" dir="ltr">{agentText}</pre>
        </Section>
      )}
    </PageShell>
  )
}

function omit(l: Live, id: string): Live {
  const { [id]: _drop, ...rest } = l
  return rest
}

function Row({ step, live, isAr, busy, onFix }: { step: StepView; live?: { status: StepStatus; detail: string }; isAr: boolean; busy: boolean; onFix: () => void }) {
  const status = live?.status ?? step.status
  const detail = live?.detail || step.detail
  const needsFix = status === "fail" || status === "blocked" || status === "human" || status === "update-available"
  const kind = step.fix?.kind
  return (
    <li className="flex items-start gap-3 rounded-2xl bg-background px-4 py-3">
      <span className={`mt-0.5 w-5 shrink-0 text-center font-semibold ${tone(status)}`} aria-hidden>{statusMark[status]}</span>
      <div className="min-w-0 flex-1">
        <p className="font-medium">{pick(step.title, isAr, step.id)}</p>
        <p className="text-xs text-muted-foreground">
          <span className={tone(status)}>{isAr ? statusWord[status].ar : statusWord[status].en}</span>
          {detail && <span dir="ltr"> · {detail}</span>}
        </p>
      </div>
      {needsFix && kind && (
        <button className={link} disabled={busy} onClick={onFix}>
          {isAr ? fixLabel[kind].ar : fixLabel[kind].en}
        </button>
      )}
    </li>
  )
}
