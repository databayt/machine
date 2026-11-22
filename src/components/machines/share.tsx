"use client"

import { useState } from "react"

import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"

import type { Copy } from "../chat/config"
import type { DonorStatus } from "../pool/actions"
import type { ShareSettings } from "../settings/types"
import { copy } from "./config"
import { duration } from "./format"

type T = (c: Copy) => string

const KINDS = ["transcribe", "render", "encode"] as const

/** The donor's line: what it is doing now, or why it is not eligible. */
export function donorLine(d: DonorStatus | null, t: T): string | null {
  if (!d) return null
  if (d.state === "running") return t(copy.stateRunning)
  if (d.state === "off") return t(copy.stateOff)
  if (d.state === "eligible") return t(copy.stateEligible)
  const r = d.reason as keyof typeof copy.reasons | null
  return r && copy.reasons[r] ? t(copy.reasons[r]) : null
}

/** "Share when idle": the opt-in, the kinds this machine accepts (only those whose tool is installed), the idle wait. */
export function SharePanel({
  share, donor, onSave, t,
}: {
  share: ShareSettings
  donor: DonorStatus | null
  onSave: (s: ShareSettings) => Promise<void>
  t: T
}) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const available = donor?.kindsAvailable ?? []
  const save = async (next: ShareSettings) => {
    setBusy(true)
    setError(null)
    try {
      await onSave(next)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }
  const toggleKind = (k: string, on: boolean) => save({ ...share, kinds: on ? [...new Set([...share.kinds, k])] : share.kinds.filter((x) => x !== k) })
  const line = donorLine(donor, t)

  return (
    <div className="bg-background flex flex-col gap-4 rounded-2xl p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <h3 className="font-semibold">{t(copy.shareTitle)}</h3>
          <p className="text-muted-foreground max-w-xl text-sm">{t(copy.shareHint)}</p>
        </div>
        <Switch checked={share.enabled} busy={busy} onCheckedChange={(on) => void save({ ...share, enabled: on })} aria-label={t(copy.shareTitle)} />
      </div>
      <fieldset className="flex flex-col gap-2" disabled={busy}>
        <legend className="text-muted-foreground mb-1 text-xs">{t(copy.kinds)}</legend>
        {KINDS.map((k) => {
          const installed = available.length === 0 && donor === null ? true : available.includes(k)
          return (
            <label key={k} className="flex items-start gap-3 text-sm">
              <input
                type="checkbox"
                className="mt-1"
                checked={share.kinds.includes(k)}
                disabled={!installed && !share.kinds.includes(k)}
                onChange={(e) => void toggleKind(k, e.target.checked)}
              />
              <span>
                <span className="font-medium">{t(copy.kindLabel[k])}</span>
                {!installed && <span className="text-muted-foreground"> · {t(copy.noTool)}</span>}
                <span className="text-muted-foreground block text-xs">{t(copy.kindHint[k])}</span>
              </span>
            </label>
          )
        })}
      </fieldset>
      <label className="flex items-center gap-3 text-sm">
        <span>{t(copy.idleMinutes)}</span>
        <Input
          type="number"
          min={1}
          max={1440}
          className="w-24"
          defaultValue={share.idleMinutes}
          key={share.idleMinutes}
          onBlur={(e) => {
            const n = Math.round(Number(e.target.value))
            if (Number.isFinite(n) && n >= 1 && n <= 1440 && n !== share.idleMinutes) void save({ ...share, idleMinutes: n })
          }}
        />
      </label>
      {(line || donor?.idleSecs !== null) && (
        <p className="text-muted-foreground text-xs" aria-live="polite">
          {t(copy.donorState)}: {line}
          {donor?.idleSecs != null && ` · ${t(copy.idle)} ${duration(donor.idleSecs)}`}
        </p>
      )}
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
    </div>
  )
}
