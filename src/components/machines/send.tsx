"use client"

import { useEffect, useState } from "react"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"

import type { Copy } from "../chat/config"
import { pickLocalFiles } from "../media/actions"
import type { MachineStatus } from "../network/types"
import { poolSubmit } from "../pool/actions"
import { POOL_KINDS, type PoolKind } from "../../lib/contract"
import { copy } from "./config"
import { buildParams, DEFAULT_FORM, type JobForm } from "./format"

type T = (c: Copy) => string

const field = "border-input bg-background h-9 w-full rounded-md border px-3 text-sm outline-none focus-visible:ring-ring/50 focus-visible:ring-[3px]"

function Labeled({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5 text-sm">
      <span className="text-muted-foreground text-xs">{label}</span>
      {children}
    </label>
  )
}

/** "Send a job": a kind, its files, its few parameters, and (optionally) the machine to run it on. */
export function SendDialog({
  open, onOpenChange, t, machines, target, onSent,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  t: T
  machines: MachineStatus[]
  /** Preselected machine (from its detail page). */
  target: string | null
  onSent: () => void
}) {
  const [kind, setKind] = useState<PoolKind>("encode")
  const [form, setForm] = useState<JobForm>(DEFAULT_FORM)
  const [paths, setPaths] = useState<string[]>([])
  const [to, setTo] = useState<string>("")
  const [busy, setBusy] = useState(false)
  const [line, setLine] = useState<string | null>(null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    if (open) {
      setTo(target ?? "")
      setLine(null)
      setFailed(false)
    }
  }, [open, target])

  const set = <K extends keyof JobForm>(k: K, v: JobForm[K]) => setForm((f) => ({ ...f, [k]: v }))
  const params = buildParams(kind, form)
  const targets = machines.filter((m) => m.share?.enabled && m.share.kinds.includes(kind))
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p

  async function pick() {
    const chosen = await pickLocalFiles(t(copy.chooseFiles), "any").catch(() => [])
    if (chosen.length) setPaths(chosen)
  }

  async function send() {
    if (paths.length === 0) return fail(t(copy.needFiles))
    if (!params) return fail(t(copy.badFrames))
    setBusy(true)
    setFailed(false)
    setLine(t(copy.sending))
    try {
      await poolSubmit(kind, params, to || null, paths, (e) => setLine(`${t(copy.sending)} ${e.index + 1}/${e.count} · ${e.total ? Math.round((e.sent / e.total) * 100) : 0}%`))
      setPaths([])
      onSent()
      onOpenChange(false)
    } catch (e) {
      fail(String(e))
    } finally {
      setBusy(false)
    }
  }
  function fail(msg: string) {
    setFailed(true)
    setLine(msg)
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !busy && onOpenChange(o)}>
      <DialogContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-1">
          <DialogTitle>{t(copy.sendJob)}</DialogTitle>
          <DialogDescription>{t(copy.kindHint[kind])}</DialogDescription>
        </div>
        <Labeled label={t(copy.kind)}>
          <select className={field} value={kind} onChange={(e) => { setKind(e.target.value as PoolKind); setTo((cur) => (cur && !machines.find((m) => m.login === cur)?.share?.kinds.includes(e.target.value) ? "" : cur)) }}>
            {POOL_KINDS.map((k) => (
              <option key={k} value={k}>{t(copy.kindLabel[k])}</option>
            ))}
          </select>
        </Labeled>
        <div className="flex flex-col gap-1.5">
          <span className="text-muted-foreground text-xs">{t(copy.files)}</span>
          <div className="flex items-center gap-3">
            <Button type="button" variant="outline" size="sm" className="rounded-full" onClick={() => void pick()} disabled={busy}>
              {t(copy.chooseFiles)}
            </Button>
            {paths.length > 0 && <span className="text-muted-foreground min-w-0 truncate text-sm" dir="auto">{paths.length === 1 ? name(paths[0]) : `${paths.length} ${t(copy.filesChosen)}`}</span>}
          </div>
        </div>

        {kind === "transcribe" && (
          <div className="grid grid-cols-3 gap-3">
            <Labeled label={t(copy.model)}>
              <select className={field} value={form.model} onChange={(e) => set("model", e.target.value)}>
                <option value="">—</option>
                {["base", "small", "medium", "large-v3"].map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            </Labeled>
            <Labeled label={t(copy.language)}>
              <Input dir="ltr" maxLength={2} placeholder={t(copy.autoLanguage)} value={form.language} onChange={(e) => set("language", e.target.value)} />
            </Labeled>
            <Labeled label={t(copy.format)}>
              <select className={field} value={form.textFormat} onChange={(e) => set("textFormat", e.target.value as JobForm["textFormat"])}>
                <option value="srt">SRT</option>
                <option value="txt">TXT</option>
              </select>
            </Labeled>
          </div>
        )}
        {kind === "render" && (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Labeled label={t(copy.engine)}>
              <select className={field} value={form.engine} onChange={(e) => set("engine", e.target.value as JobForm["engine"])}>
                <option value="eevee">EEVEE</option>
                <option value="cycles">Cycles</option>
              </select>
            </Labeled>
            <Labeled label={`${t(copy.frames)}: ${t(copy.from)}`}>
              <Input dir="ltr" inputMode="numeric" value={form.frameStart} onChange={(e) => set("frameStart", e.target.value)} />
            </Labeled>
            <Labeled label={t(copy.to)}>
              <Input dir="ltr" inputMode="numeric" value={form.frameEnd} onChange={(e) => set("frameEnd", e.target.value)} />
            </Labeled>
            <Labeled label={t(copy.format)}>
              <select className={field} value={form.renderFormat} onChange={(e) => set("renderFormat", e.target.value as JobForm["renderFormat"])}>
                <option value="mp4">MP4</option>
                <option value="png">PNG</option>
              </select>
            </Labeled>
          </div>
        )}
        {kind === "encode" && (
          <Labeled label={t(copy.preset)}>
            <select className={field} value={form.preset} onChange={(e) => set("preset", e.target.value as JobForm["preset"])}>
              {(Object.keys(copy.presets) as (keyof typeof copy.presets)[]).map((p) => <option key={p} value={p}>{t(copy.presets[p])}</option>)}
            </select>
          </Labeled>
        )}

        <Labeled label={t(copy.target)}>
          <select className={field} value={to} onChange={(e) => setTo(e.target.value)}>
            <option value="">{t(copy.anyMachine)}</option>
            {/* A chosen target stays listed even if it stopped sharing: kun decides on claim. */}
            {[...targets, ...machines.filter((m) => m.login === to && !targets.includes(m))].map((m) => <option key={m.login} value={m.login}>{m.name}</option>)}
          </select>
        </Labeled>

        {line && <p role={failed ? "alert" : "status"} dir="auto" className={failed ? "text-destructive text-sm" : "text-muted-foreground text-sm"}>{line}</p>}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" className="rounded-full" disabled={busy} onClick={() => onOpenChange(false)}>{t(copy.close)}</Button>
          <Button type="button" className="rounded-full" disabled={busy} onClick={() => void send()}>{t(copy.send)}</Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
