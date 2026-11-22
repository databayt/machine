"use client"

import { useState } from "react"

import { Button } from "@/components/ui/button"

import type { Copy } from "../chat/config"
import { poolCancel, poolDownload, type PoolJob } from "../pool/actions"
import { copy } from "./config"
import { canCancel, isDone } from "./format"

type T = (c: Copy) => string

function JobCard({ j, t, onChange }: { j: PoolJob; t: T; onChange: () => void }) {
  const [busy, setBusy] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const act = async (f: () => Promise<unknown>, ok?: string) => {
    setBusy(true)
    setNote(null)
    try {
      await f()
      if (ok) setNote(ok)
      onChange()
    } catch (e) {
      setNote(String(e))
    } finally {
      setBusy(false)
    }
  }
  const status = copy.status[j.status as keyof typeof copy.status]
  const pct = Math.round(j.progress * 100)
  return (
    <li className="bg-background flex flex-col gap-2 rounded-2xl p-4">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col">
          <span className="truncate font-semibold">{t(copy.kindLabel[j.kind as keyof typeof copy.kindLabel] ?? { en: j.kind, ar: j.kind })}</span>
          <span className="text-muted-foreground truncate text-xs">
            {status ? t(status) : j.status}
            {j.donor ? ` · ${t(copy.by)} ${j.donor}` : j.status === "queued" ? ` · ${t(copy.waiting)}` : ""}
            {j.target && !j.donor ? ` → ${j.target}` : ""}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {isDone(j.status) && (
            <Button size="sm" className="rounded-full" disabled={busy} onClick={() => void act(() => poolDownload(j.id), t(copy.downloaded))}>
              {t(copy.download)}
            </Button>
          )}
          {canCancel(j.status) && (
            <Button size="sm" variant="ghost" className="rounded-full" disabled={busy} onClick={() => void act(() => poolCancel(j.id))}>
              {t(copy.cancel)}
            </Button>
          )}
        </div>
      </div>
      {j.status === "running" && (
        <div role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} className="bg-muted h-1.5 overflow-hidden rounded-full">
          <div className="bg-foreground h-full rounded-full transition-[width]" style={{ width: `${pct}%` }} />
        </div>
      )}
      {j.status === "failed" && j.log && <p dir="auto" className="text-destructive line-clamp-2 text-xs">{j.log.trim().split("\n").pop()}</p>}
      {note && <p dir="auto" className="text-muted-foreground text-xs">{note}</p>}
    </li>
  )
}

/** My jobs: kind, status, progress, who runs it, Download when done, Cancel while it can still be stopped. */
export function Jobs({ jobs, t, onChange, onSend }: { jobs: PoolJob[]; t: T; onChange: () => void; onSend: () => void }) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <h2 className="font-semibold">{t(copy.jobs)}</h2>
        <Button size="sm" variant="outline" className="rounded-full" onClick={onSend}>
          {t(copy.sendJob)}
        </Button>
      </div>
      {jobs.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t(copy.noJobs)}</p>
      ) : (
        <ul className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {jobs.map((j) => <JobCard key={j.id} j={j} t={t} onChange={onChange} />)}
        </ul>
      )}
    </section>
  )
}
