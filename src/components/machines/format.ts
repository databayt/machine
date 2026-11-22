// machines — pure helpers (no React): the one-line resource summary, the pool totals, the gate line
// and the job parameters a form builds. Node tests them (format.test.ts).
import type { MachineStatus, Resources } from "../network/types.ts"
import { poolParamsSchemas, type PoolKind } from "../../lib/contract.ts"

const GIB = 1024 ** 3

/** Whole GB, one decimal under 10: 48 GB, 7.5 GB. */
export function gb(bytes: number | undefined | null): string {
  if (bytes === undefined || bytes === null || !Number.isFinite(bytes)) return "?"
  const v = bytes / GIB
  return `${v >= 10 ? Math.round(v) : Math.round(v * 10) / 10} GB`
}

/** A short processor name: "Apple M4 Pro" → "M4 Pro", "AMD Ryzen 9 7950X 16-Core Processor" → "Ryzen 9 7950X". */
export function cpuName(model: string): string {
  return model
    .replace(/\((R|TM)\)/gi, "")
    .replace(/\s*(CPU\s*)?@.*$/i, "")
    .replace(/\b\d+-Core\b/i, "")
    .replace(/\bProcessor\b/i, "")
    .replace(/^(Apple|AMD|Intel Core|Intel)\s+/i, "")
    .replace(/\s+/g, " ")
    .trim()
}

/** "RTX 4090" from "NVIDIA GeForce RTX 4090". */
export function gpuName(name: string): string {
  return name.replace(/^(NVIDIA|AMD|Intel)\s+/i, "").replace(/^(GeForce|Radeon)\s+/i, "").trim()
}

const words = {
  en: { free: "free", onPower: "on power", battery: "battery", core: "-core GPU", none: "No readings yet" },
  ar: { free: "متاح", onPower: "على الكهرباء", battery: "بطارية", core: " نواة رسومية", none: "لا قراءات بعد" },
}

/** "M4 Pro · 48 GB · 20-core GPU · 120 GB free · on power" */
export function summary(r: Resources | null | undefined, isAr: boolean): string {
  const w = words[isAr ? "ar" : "en"]
  if (!r) return w.none
  const parts: string[] = []
  if (r.cpu) parts.push(cpuName(r.cpu.model))
  if (r.ramTotal !== undefined) parts.push(gb(r.ramTotal))
  const g = r.gpu?.[0]
  if (g) parts.push(g.cores ? `${g.cores}${w.core}` : gpuName(g.name))
  if (r.diskFree !== undefined) parts.push(`${gb(r.diskFree)} ${w.free}`)
  if (r.power) parts.push(r.power.onAc ? w.onPower : `${w.battery}${r.power.batteryPercent !== undefined ? ` ${Math.round(r.power.batteryPercent)}%` : ""}`)
  return parts.length ? parts.join(" · ") : w.none
}

export interface PoolTotals {
  online: number
  sharing: number
  ramFree: number
  gpus: number
}

/** The pool row: machines online, sharing now, their free memory and GPUs. Offline machines count for nothing. */
export function poolTotals(machines: MachineStatus[]): PoolTotals {
  const t: PoolTotals = { online: 0, sharing: 0, ramFree: 0, gpus: 0 }
  for (const m of machines) {
    if (!m.online) continue
    t.online += 1
    if (m.share?.enabled) t.sharing += 1
    t.ramFree += m.resources?.ramFree ?? 0
    t.gpus += m.resources?.gpu?.length ?? 0
  }
  return t
}

/** "ready to contribute", or how many steps of the doctor are left; null when the gate never ran. */
export function gateLine(gate: MachineStatus["gate"]): { pass: boolean; left: number } | null {
  if (!gate) return null
  return { pass: gate.pass, left: gate.failing.length }
}

/** A relative time ("5 minutes ago"); "" for no date. */
export function ago(iso: string | null | undefined, now: number, lang: string): string {
  if (!iso) return ""
  const secs = Math.round((new Date(iso).getTime() - now) / 1000)
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" })
  const abs = Math.abs(secs)
  if (abs < 60) return rtf.format(Math.round(secs / 1), "second")
  if (abs < 3600) return rtf.format(Math.round(secs / 60), "minute")
  if (abs < 86400) return rtf.format(Math.round(secs / 3600), "hour")
  return rtf.format(Math.round(secs / 86400), "day")
}

/** "2 min" / "1 h 5 min" for an idle time in seconds. */
export function duration(secs: number | undefined): string {
  if (secs === undefined || !Number.isFinite(secs)) return "?"
  if (secs < 60) return `${Math.floor(secs)} s`
  const m = Math.floor(secs / 60)
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`
}

// ---- job parameters ------------------------------------------------------------------------------

export interface JobForm {
  model: string
  language: string
  textFormat: "srt" | "txt"
  engine: "eevee" | "cycles"
  frameStart: string
  frameEnd: string
  renderFormat: "png" | "mp4"
  preset: "h264-web" | "hevc" | "proxy"
}

export const DEFAULT_FORM: JobForm = {
  model: "",
  language: "",
  textFormat: "srt",
  engine: "eevee",
  frameStart: "1",
  frameEnd: "1",
  renderFormat: "mp4",
  preset: "h264-web",
}

/** The parameters for a kind, checked with the same schema kun uses. `null` means the form is not valid yet. */
export function buildParams(kind: PoolKind, f: JobForm): Record<string, unknown> | null {
  let raw: Record<string, unknown>
  if (kind === "transcribe") {
    raw = { format: f.textFormat, ...(f.model ? { model: f.model } : {}), ...(f.language ? { language: f.language.trim().toLowerCase() } : {}) }
  } else if (kind === "render") {
    raw = { engine: f.engine, frameStart: Number(f.frameStart), frameEnd: Number(f.frameEnd), format: f.renderFormat }
  } else {
    raw = { preset: f.preset }
  }
  const parsed = poolParamsSchemas[kind].safeParse(raw)
  return parsed.success ? (parsed.data as Record<string, unknown>) : null
}

/** Which kinds this machine's tools can run, for a requester there is no such limit. */
export function kindEnabled(kind: PoolKind, available: string[] | undefined): boolean {
  return !available || available.includes(kind)
}

export const canCancel = (status: string) => status === "uploading" || status === "queued" || status === "running"
export const isDone = (status: string) => status === "done"
