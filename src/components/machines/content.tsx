"use client"

import { useSearchParams } from "next/navigation"
import { useMemo, useState } from "react"

import { tr } from "@/components/chat/config"
import type { Locale } from "@/components/local/config"

import { Grid } from "../plugins/card"
import { DeptPills, SearchField, useDeptFilter } from "../plugins/filters"
import { OpenInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { MachineCard } from "./card"
import { copy } from "./config"
import { MachineDetail } from "./detail"
import { gb, poolTotals } from "./format"
import { Jobs } from "./jobs"
import { SendDialog } from "./send"
import { SharePanel } from "./share"
import { useMachines } from "./use-machines"

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-background flex min-w-0 flex-col gap-0.5 rounded-2xl p-4">
      <span className="text-2xl font-semibold tabular-nums">{value}</span>
      <span className="text-muted-foreground truncate text-xs">{label}</span>
    </div>
  )
}

/** `/machines/` → the team; `?item=<login>` → one machine. Read in the client (static export). */
export default function Machines({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const filter = useDeptFilter()
  const names = useMemo(() => filter.options.map((o) => o.name), [filter.options])
  const m = useMachines(names)
  const item = useSearchParams().get("item")
  const [query, setQuery] = useState("")
  const [sendOpen, setSendOpen] = useState(false)
  const [target, setTarget] = useState<string | null>(null)
  const openSend = (to: string | null) => {
    setTarget(to)
    setSendOpen(true)
  }

  if (m.inApp === null) return null
  if (!m.inApp)
    return (
      <PageShell title={t(copy.title)}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  const machines = m.machines ?? []
  const dialog = <SendDialog open={sendOpen} onOpenChange={setSendOpen} t={t} machines={machines} target={target} onSent={() => void m.reloadJobs()} />
  const banner = m.finished.length > 0 && (
    <p role="status" className="bg-background flex items-center justify-between gap-3 rounded-2xl p-3 text-sm">
      <span>{t(copy.jobDone)}</span>
      <button type="button" onClick={m.dismiss} className="text-muted-foreground hover:text-foreground text-xs underline underline-offset-4">{t(copy.close)}</button>
    </p>
  )
  const jobs = <Jobs jobs={m.jobs?.mine ?? []} t={t} onChange={() => void m.reloadJobs()} onSend={() => openSend(null)} />

  if (item) {
    const one = machines.find((x) => x.login === item)
    const self = !!m.me && m.me.login === item
    return (
      <PageShell title={one?.name ?? t(copy.title)}>
        {m.machines === null ? (
          <p className="text-muted-foreground text-sm">{t(m.failed ? copy.failed : copy.loading)}</p>
        ) : one ? (
          <>
            {banner}
            <MachineDetail m={one} lang={lang} t={t} canSend onSend={() => openSend(one.login)} />
            {self && m.settings && <SharePanel share={m.settings.share} donor={m.donor} onSave={m.saveShare} t={t} />}
            {jobs}
          </>
        ) : (
          <div className="bg-background text-muted-foreground w-fit rounded-2xl p-4 text-sm">{t(copy.notFound)}</div>
        )}
        {dialog}
      </PageShell>
    )
  }

  const q = query.trim().toLowerCase()
  const shown = machines.filter((x) => (filter.value === "all" || x.departments.includes(filter.value) || (m.me?.login === x.login && x.departments.length === 0)) && (!q || `${x.name} ${x.login}`.toLowerCase().includes(q)))
  const totals = poolTotals(machines)

  return (
    <PageShell title={t(copy.title)} className="max-w-6xl">
      {banner}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <Stat label={t(copy.poolOnline)} value={String(totals.online)} />
        <Stat label={t(copy.poolSharing)} value={String(totals.sharing)} />
        <Stat label={t(copy.poolRam)} value={gb(totals.ramFree)} />
        <Stat label={t(copy.poolGpus)} value={String(totals.gpus)} />
      </div>
      <div className="flex flex-col gap-4">
        <DeptPills options={filter.options} value={filter.value} onChange={filter.set} t={t} />
        <SearchField value={query} onChange={setQuery} placeholder={t(copy.search)} />
      </div>
      {m.machines === null ? (
        <p className="text-muted-foreground text-sm">{t(m.failed ? copy.failed : copy.loading)}</p>
      ) : shown.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>
      ) : (
        <Grid>
          {shown.map((x) => {
            const self = m.me?.login === x.login
            return (
              <MachineCard
                key={x.login}
                m={x}
                lang={lang}
                t={t}
                self={self}
                sharing={self ? (m.settings?.share.enabled ?? false) : !!x.share?.enabled}
                onShare={self && m.settings ? (on) => void m.saveShare({ ...m.settings!.share, enabled: on }).catch(() => {}) : undefined}
              />
            )
          })}
        </Grid>
      )}
      {jobs}
      {dialog}
    </PageShell>
  )
}
