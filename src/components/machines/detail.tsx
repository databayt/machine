"use client"

import { Button } from "@/components/ui/button"

import type { Copy } from "../chat/config"
import type { MachineStatus } from "../network/types"
import { Logo } from "../plugins/card"
import { OnlineDot, osAndDepartments } from "./card"
import { copy } from "./config"
import { ago, cpuName, duration, gateLine, gb, gpuName } from "./format"

type T = (c: Copy) => string

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[8rem_1fr] gap-3 py-2 text-sm sm:grid-cols-[10rem_1fr]">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </div>
  )
}

/** One machine, every reading as a row, and a button to send it a job. */
export function MachineDetail({ m, lang, t, onSend, canSend }: { m: MachineStatus; lang: string; t: T; onSend: () => void; canSend: boolean }) {
  const r = m.resources
  const gate = gateLine(m.gate)
  const unknown = <span className="text-muted-foreground">?</span>
  const net = r?.internet
  return (
    <>
      <div className="flex flex-wrap items-center gap-4">
        <Logo c={{ id: m.login, name: m.name, blurb: copy.title }} size={64} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <h2 className="flex items-center gap-2 text-xl font-semibold">
            <bdi className="break-all">{m.name}</bdi>
            <OnlineDot online={m.online} t={t} />
          </h2>
          <p className="text-muted-foreground text-sm">{osAndDepartments(m, t)}</p>
          {gate && (
            <p className={gate.pass ? "text-sm text-emerald-600 dark:text-emerald-400" : "text-sm text-amber-600 dark:text-amber-400"}>
              {gate.pass ? t(copy.gateReady) : `${t(copy.gateLeft)}: ${gate.left} ${t(copy.gateLeftN)}`}
            </p>
          )}
        </div>
        {canSend && (
          <Button onClick={onSend} className="rounded-full">
            {t(copy.sendHere)}
          </Button>
        )}
      </div>
      <dl className="bg-background divide-border flex flex-col divide-y rounded-2xl px-4 py-2">
        <Row label={t(copy.cpu)}>{r?.cpu ? <bdi>{`${cpuName(r.cpu.model)} · ${r.cpu.cores}`}</bdi> : unknown}</Row>
        <Row label={t(copy.memory)}>{r?.ramTotal !== undefined ? `${gb(r.ramTotal)}${r.ramFree !== undefined ? ` · ${gb(r.ramFree)} ${t(copy.free)}` : ""}` : unknown}</Row>
        <Row label={t(copy.graphics)}>
          {r?.gpu ? (r.gpu.length ? r.gpu.map((g) => <div key={g.name}><bdi>{gpuName(g.name)}{g.cores ? ` · ${g.cores}${t(copy.gpuCores)}` : ""}{g.vramBytes ? ` · ${gb(g.vramBytes)}` : ""}</bdi></div>) : t(copy.none)) : unknown}
        </Row>
        <Row label={t(copy.disk)}>{r?.diskFree !== undefined ? gb(r.diskFree) : unknown}</Row>
        <Row label={t(copy.drive)}>
          {r?.drive ? (r.drive.installed ? <bdi>{[r.drive.account, r.drive.usedBytes !== undefined ? `${gb(r.drive.usedBytes)} ${t(copy.driveUsed)}` : null].filter(Boolean).join(" · ") || "✓"}</bdi> : t(copy.driveNone)) : unknown}
        </Row>
        <Row label={t(copy.internet)}>
          {net ? <bdi>{[net.online ? t(copy.online) : t(copy.offline), net.latencyMs !== undefined ? `${Math.round(net.latencyMs)} ms` : null, net.mbpsDown !== undefined ? `${net.mbpsDown} Mbps` : null].filter(Boolean).join(" · ")}</bdi> : unknown}
        </Row>
        <Row label={t(copy.power)}>{r?.power ? (r.power.onAc ? t(copy.onPower) : `${t(copy.onBattery)}${r.power.batteryPercent !== undefined ? ` ${Math.round(r.power.batteryPercent)}%` : ""}`) : unknown}</Row>
        <Row label={t(copy.idle)}>{r?.idleSecs !== undefined ? duration(r.idleSecs) : unknown}</Row>
        <Row label={t(copy.lastSeen)}>{m.lastSeenAt ? ago(m.lastSeenAt, Date.now(), lang) : t(copy.never)}</Row>
        <Row label={t(copy.currentTask)}>{m.currentTask ? <bdi>{m.currentTask}</bdi> : t(copy.none)}</Row>
        <Row label={t(copy.readiness)}>{m.readinessPercent !== null ? `${Math.round(m.readinessPercent)}%` : unknown}</Row>
        <Row label={t(copy.sharing)}>
          {m.share?.enabled ? `${t(copy.sharingOn)}${m.share.kinds.length ? ` · ${m.share.kinds.map((k) => t(copy.kindLabel[k as keyof typeof copy.kindLabel])).join(", ")}` : ""}` : t(copy.sharingOff)}
        </Row>
      </dl>
    </>
  )
}
