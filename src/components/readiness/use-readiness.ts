"use client"

// readiness — the one hook behind the install wizard's three screens: the catalog view, live step
// statuses from run events, and every action (check, complete, maintain, fix, heal, uninstall).
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { useInMachine } from "../shell/content"
import {
  healWithAgent,
  readinessCancel,
  readinessDoctor,
  readinessDoctorFix,
  readinessFix,
  readinessRun,
  readinessState,
  readinessUninstall,
  readinessUninstallTerminal,
} from "./actions"
import { wizard } from "./config"
import type { ReadinessView, RunEvent, Stage, StepStatus, StepView } from "./types"

export type Running = "check" | "complete" | "maintain" | "fix" | "heal" | "uninstall" | "doctor" | "doctorFix"

type Live = Record<string, { status: StepStatus; detail: string }>

export const PASSING: StepStatus[] = ["pass", "fixed", "updated"]
const NEEDS_WORK: StepStatus[] = ["fail", "blocked", "human"]

export const isPassing = (s: StepStatus) => PASSING.includes(s)
export const needsWork = (s: StepStatus) => NEEDS_WORK.includes(s)

/** The catalog's stages in order, plus any stage a step names that the catalog list does not. */
export function stageList(view: ReadinessView | null): Stage[] {
  if (!view) return []
  const order = view.stages.map((s) => s.id)
  const extra = [...new Set(view.steps.map((s) => s.stage))].filter((s) => !order.includes(s))
  return [...view.stages, ...extra.map((id) => ({ id, title: null }))].filter((s) => view.steps.some((x) => x.stage === s.id))
}

/** Steps that still need something, in a stage, as of the given (merged) steps. */
export function stageNeedsWork(steps: StepView[], removed: string[], stageId: string): boolean {
  return steps.some((s) => s.stage === stageId && !removed.includes(s.id) && needsWork(s.status))
}

export function useReadiness(isAr: boolean) {
  const inApp = useInMachine()
  const [view, setView] = useState<ReadinessView | null>(null)
  const [live, setLive] = useState<Live>({})
  const [percent, setPercent] = useState<number | null>(null)
  const [running, setRunning] = useState<Running | null>(null)
  const [progress, setProgress] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [agentText, setAgentText] = useState("")
  const [log, setLog] = useState<{ id: string | null; text: string }[]>([])
  const mounted = useRef(true)

  const load = useCallback(async (): Promise<ReadinessView | null> => {
    const v = await readinessState()
    if (!mounted.current || !v) return null
    setView(v)
    setPercent(v.percent)
    return v
  }, [])

  useEffect(() => {
    mounted.current = true
    if (inApp) load().catch((e) => setError(String(e)))
    return () => {
      mounted.current = false
    }
  }, [inApp, load])

  // The automatic doctor (launch, update) streams its steps and ends with the verdict: follow both.
  useEffect(() => {
    if (!inApp) return
    let offs: Array<() => void> = []
    let live = true
    import("@tauri-apps/api/event")
      .then(async (m) => [
        await m.listen<RunEvent>("readiness:event", (e) => {
          const ev = e.payload
          if (ev.type === "step") setLive((l) => ({ ...l, [ev.id]: { status: ev.status, detail: ev.detail } }))
        }),
        await m.listen("readiness:gate", () => {
          setLive({})
          void load()
        }),
      ])
      .then((fs) => (live ? (offs = fs) : fs.forEach((f) => f())))
      .catch(() => {})
    return () => {
      live = false
      offs.forEach((f) => f())
    }
  }, [inApp, load])

  const onEvent = useCallback((e: RunEvent) => {
    if (e.type === "step") {
      setLive((l) => ({ ...l, [e.id]: { status: e.status, detail: e.detail } }))
      setLog((l) => [...l, { id: e.id, text: `${e.status}${e.detail ? ` · ${e.detail}` : ""}` }].slice(-200))
    } else if (e.type === "progress") setProgress(e.label)
    else if (e.type === "start") setProgress(`${e.mode} · ${e.total}`)
    else if (e.type === "done") setPercent(e.percent)
    else if (e.type === "log") setLog((l) => [...l, { id: null, text: e.line }].slice(-200))
    else if (e.type === "error") setError(e.message)
  }, [])

  /** Wraps one action: busy flag, fresh errors, a reload after, and the sidebar nudge. */
  const guarded = useCallback(
    async <T,>(kind: Running, fn: () => Promise<T>): Promise<T | null> => {
      setRunning(kind)
      setError(null)
      setNotice(null)
      setLog([])
      try {
        const out = await fn()
        await load()
        setLive({})
        return out
      } catch (e) {
        setError(String(e))
        setLive({})
        return null
      } finally {
        setRunning(null)
        setProgress(null)
        window.dispatchEvent(new Event("kun:readiness"))
      }
    },
    [load],
  )

  const run = useCallback(
    (mode: "check" | "complete" | "maintain", only?: string[]) => guarded(mode, () => readinessRun(mode, onEvent, only)),
    [guarded, onEvent],
  )

  /** The doctor: re-check only the gate steps. */
  const doctor = useCallback(() => guarded("doctor", () => readinessDoctor(onEvent)), [guarded, onEvent])

  /** The doctor's automatic fixes for the failing gate steps. */
  const doctorFix = useCallback(() => guarded("doctorFix", () => readinessDoctorFix(onEvent)), [guarded, onEvent])

  const fix = useCallback(
    (id: string) =>
      guarded("fix", async () => {
        setLive((l) => ({ ...l, [id]: { status: "fixing", detail: "" } }))
        const out = await readinessFix(id, onEvent)
        if (out.message) setNotice(out.message)
        return out
      }),
    [guarded, onEvent],
  )

  /** Installs a step the person removed: clear the opt-out through a run, then fall back to its fix. */
  const installRemoved = useCallback(
    (id: string) =>
      guarded("complete", async () => {
        await readinessRun("complete", onEvent, [id])
        const v = await readinessState()
        const s = v?.steps.find((x) => x.id === id)
        if (s && !isPassing(s.status) && s.fix && s.fix.kind !== "auto") {
          const out = await readinessFix(id, onEvent)
          if (out.message) setNotice(out.message)
        }
      }),
    [guarded, onEvent],
  )

  /** The one action a step's button does: install (or sign in, open, ask) or update. */
  const act = useCallback(
    (step: StepView) => {
      if (view?.removed.includes(step.id)) return installRemoved(step.id)
      if (step.status === "update-available") return run("maintain", [step.id])
      return fix(step.id)
    },
    [view, installRemoved, run, fix],
  )

  const heal = useCallback(
    (ids: string[]) =>
      guarded("heal", async () => {
        setAgentText("")
        await healWithAgent(ids, (e) => {
          if (e.kind === "text") setAgentText((s) => s + e.text)
          else if (e.kind === "error") setError(e.message)
        })
      }),
    [guarded],
  )

  const uninstall = useCallback(
    (id: string) =>
      guarded("uninstall", async () => {
        const out = await readinessUninstall([id], onEvent)
        const bad = [...out.blocked, ...out.failed].map((n) => `${n.id}: ${n.detail}`)
        if (bad.length) setError(bad.join("\n"))
        else if (out.handoff.length) {
          await readinessUninstallTerminal(id)
          setNotice(isAr ? wizard.terminalOpen.ar : wizard.terminalOpen.en)
        } else if (out.cancelled) setNotice(isAr ? wizard.cancelled.ar : wizard.cancelled.en)
        else if (out.removed.length) setNotice(isAr ? wizard.removedOk.ar : wizard.removedOk.en)
        return out
      }),
    [guarded, onEvent, isAr],
  )

  const cancel = useCallback(() => readinessCancel().catch(() => false), [])

  /** The view's steps with live run statuses laid over them. */
  const steps = useMemo<StepView[]>(
    () => (view?.steps ?? []).map((s) => (live[s.id] ? { ...s, status: live[s.id].status, detail: live[s.id].detail || s.detail } : s)),
    [view, live],
  )

  return {
    inApp,
    view,
    steps,
    removed: view?.removed ?? [],
    percent,
    running,
    busy: running !== null,
    progress,
    error,
    notice,
    agentText,
    log,
    load,
    run,
    doctor,
    doctorFix,
    fix,
    act,
    heal,
    uninstall,
    cancel,
  }
}

export type Readiness = ReturnType<typeof useReadiness>
