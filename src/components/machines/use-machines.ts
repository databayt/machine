"use client"

// machines — the one hook behind the page: the team from kun, this machine's settings and donor
// state, and my pool jobs. It refreshes every 30 s (every 5 s while a job of mine is moving) and
// when the network plugin says a message arrived (kun writes one when a job of mine finishes).
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { getTeam } from "../network/actions"
import type { MachineStatus } from "../network/types"
import { poolJobs, poolStatus, type DonorStatus, type PoolList } from "../pool/actions"
import { getProfile } from "../profile/actions"
import { loadSettings, saveSettings } from "../settings/actions"
import type { Settings, ShareSettings } from "../settings/types"
import { useInMachine } from "../shell/content"

export interface Me {
  login: string
  name: string
  os: string
}

const SLOW = 30_000
const FAST = 5_000

export function useMachines(departments: string[]) {
  const inApp = useInMachine()
  const [machines, setMachines] = useState<MachineStatus[] | null>(null)
  const [failed, setFailed] = useState(false)
  const [me, setMe] = useState<Me | null>(null)
  const [settings, setSettings] = useState<Settings | null>(null)
  const [donor, setDonor] = useState<DonorStatus | null>(null)
  const [jobs, setJobs] = useState<PoolList | null>(null)
  /** Ids of my jobs that turned done since the page opened and were not dismissed. */
  const [finished, setFinished] = useState<string[]>([])
  const seen = useRef<Map<string, string>>(new Map())
  const key = departments.join(",")

  const loadTeam = useCallback(async () => {
    const answers = await Promise.all(departments.map((d) => getTeam(d).catch(() => null)))
    const ok = answers.filter((a) => a !== null)
    if (ok.length === 0) {
      setFailed(true)
      return
    }
    setFailed(false)
    const byLogin = new Map<string, MachineStatus>()
    for (const a of ok) for (const m of a.machines) byLogin.set(m.login, m)
    setMachines([...byLogin.values()].sort((a, b) => Number(b.online) - Number(a.online) || a.name.localeCompare(b.name)))
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` stands for the list
  }, [key])

  const loadMine = useCallback(async () => {
    const [list, status] = await Promise.all([poolJobs().catch(() => null), poolStatus().catch(() => null)])
    if (list) {
      const news: string[] = []
      for (const j of list.mine) {
        const before = seen.current.get(j.id)
        if (before && before !== "done" && j.status === "done") news.push(j.id)
        seen.current.set(j.id, j.status)
      }
      if (news.length) setFinished((f) => [...new Set([...f, ...news])])
      setJobs(list)
    }
    if (status) setDonor(status)
  }, [])

  const loadSelf = useCallback(async () => {
    const [p, s] = await Promise.all([getProfile().catch(() => null), loadSettings().catch(() => null)])
    if (s) setSettings(s)
    if (p) setMe({ login: s?.login || p.login, name: p.name, os: p.os })
  }, [])

  useEffect(() => {
    if (!inApp) return
    void loadSelf()
    void loadTeam()
    void loadMine()
  }, [inApp, loadSelf, loadTeam, loadMine])

  const moving = jobs?.mine.some((j) => j.status === "uploading" || j.status === "queued" || j.status === "running") ?? false
  useEffect(() => {
    if (!inApp) return
    const id = window.setInterval(() => {
      void loadMine()
      void loadTeam()
    }, moving || donor?.state === "running" ? FAST : SLOW)
    return () => window.clearInterval(id)
  }, [inApp, moving, donor?.state, loadMine, loadTeam])

  useEffect(() => {
    if (!inApp) return
    let off: (() => void) | undefined
    let live = true
    import("@tauri-apps/api/event")
      .then((m) => m.listen("network:unread", () => void loadMine()))
      .then((f) => (live ? (off = f) : f()))
      .catch(() => {})
    return () => {
      live = false
      off?.()
    }
  }, [inApp, loadMine])

  /** The team, with this machine added when kun has not heard from it yet. */
  const all = useMemo<MachineStatus[] | null>(() => {
    if (!machines) return null
    if (!me || machines.some((m) => m.login === me.login)) return machines
    const self: MachineStatus = {
      login: me.login, name: me.name || me.login, os: me.os, departments: [], lastSeenAt: null, healthOk: null, currentTask: null,
      resources: null, share: settings?.share ?? null, online: true, readinessPercent: null, gate: null,
    }
    return [self, ...machines]
  }, [machines, me, settings])

  /** Saves the pool opt-in and refreshes the donor state. */
  const saveShare = useCallback(
    async (share: ShareSettings) => {
      const now = settings ?? (await loadSettings())
      if (!now) return
      const saved = await saveSettings({ ...now, share })
      setSettings(saved)
      await loadMine()
    },
    [settings, loadMine],
  )

  return {
    inApp, machines: all, failed, me, settings, donor, jobs, finished,
    dismiss: () => setFinished([]),
    reload: () => Promise.all([loadTeam(), loadMine()]).then(() => undefined),
    reloadJobs: loadMine,
    saveShare,
  }
}
