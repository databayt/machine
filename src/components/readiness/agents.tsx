"use client"

import { useCallback, useEffect, useState } from "react"

import type { Locale } from "@/components/local/config"

import { cn } from "@/lib/utils"
import { loadSettings, saveSettings } from "../settings/actions"
import type { Settings } from "../settings/types"
import { getProfile, type MachineProfile } from "../profile/actions"
import { OpenInMachine, useInMachine } from "../shell/content"
import { agentsTools, readinessFix, readinessRun, type AgentToolRow } from "./actions"
import { statusWord, tone } from "./config"

const link = "text-sm font-medium text-primary underline-offset-4 hover:underline disabled:opacity-50"

/** The Agents & tools tab: installed or not, install and update per row, and the default agent. */
export default function AgentsContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (en: string, ar: string) => (isAr ? ar : en)
  const inApp = useInMachine()
  const [rows, setRows] = useState<AgentToolRow[] | null>(null)
  const [settings, setSettings] = useState<Settings | null>(null)
  const [profile, setProfile] = useState<MachineProfile | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  const reload = useCallback(() => agentsTools().then(setRows).catch((e) => setError(String(e))), [])

  useEffect(() => {
    if (!inApp) return
    void reload()
    loadSettings().then(setSettings).catch(() => setSettings(null))
    getProfile().then(setProfile).catch(() => setProfile(null))
  }, [inApp, reload])

  async function act(row: AgentToolRow, what: "fix" | "update") {
    if (!row.step_id) return
    setBusy(`${row.id}:${what}`)
    setError(null)
    try {
      if (what === "fix") {
        const out = await readinessFix(row.step_id)
        if (out.message) setError(out.message)
      } else {
        await readinessRun("maintain", undefined, [row.step_id])
      }
      await reload()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  async function change(next: Settings) {
    setSettings(next)
    setSaved(false)
    try {
      setSettings(await saveSettings(next))
      setSaved(true)
    } catch (e) {
      setError(String(e))
    }
  }

  if (inApp === null) return null
  if (!inApp) return <OpenInMachine lang={lang} />

  const agents = (rows ?? []).filter((r) => r.kind === "agent")
  const tools = (rows ?? []).filter((r) => r.kind === "tool")
  const locked = profile?.driver

  return (
    <div className="mx-auto w-full max-w-3xl space-y-6 px-4 py-6">
      <h1 className="text-balance text-2xl font-semibold tracking-tight">{t("Agents & tools", "الوكلاء والأدوات")}</h1>
      {error && <p className="text-sm text-destructive">{error}</p>}

      {settings && (
        <section className="space-y-3 rounded-2xl bg-muted/70 p-4">
          <h2 className="text-sm font-semibold">{t("Default agent", "الوكيل الافتراضي")}</h2>
          {locked ? (
            <p dir="ltr" className="text-sm font-medium">
              {locked} <span className="text-xs text-muted-foreground">· {t("set by this machine's profile", "يحدده ملف جهازك")}</span>
            </p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {agents.map((a) => (
                <button
                  key={a.id}
                  type="button"
                  disabled={!a.installed}
                  aria-pressed={settings.driver === a.id}
                  onClick={() => change({ ...settings, driver: a.id as Settings["driver"] })}
                  className={cn("rounded-full bg-background px-4 py-1.5 text-sm disabled:opacity-50", settings.driver === a.id && "bg-foreground text-background")}
                >
                  {a.label}
                </button>
              ))}
            </div>
          )}
          <label className="flex items-start gap-3 text-sm">
            <input type="checkbox" className="mt-1" checked={settings.autoRun} onChange={(e) => change({ ...settings, autoRun: e.target.checked })} />
            <span>
              <span className="font-medium">{t("Pick up auto tasks", "التقاط المهام تلقائيًا")}</span>
              <span className="block text-xs text-muted-foreground">{t("Only tasks labelled run:auto.", "المهام الموسومة run:auto فقط.")}</span>
            </span>
          </label>
          {saved && <p className="text-xs text-muted-foreground">{t("Saved.", "تم الحفظ.")}</p>}
        </section>
      )}

      <Group title={t("Agents", "الوكلاء")} rows={agents} isAr={isAr} busy={busy} onAct={act} />
      <Group title={t("Production tools", "أدوات الإنتاج")} rows={tools} isAr={isAr} busy={busy} onAct={act} />
    </div>
  )
}

function Group({ title, rows, isAr, busy, onAct }: { title: string; rows: AgentToolRow[]; isAr: boolean; busy: string | null; onAct: (r: AgentToolRow, w: "fix" | "update") => void }) {
  if (rows.length === 0) return null
  return (
    <section className="space-y-2">
      <h2 className="text-sm font-semibold">{title}</h2>
      <ul className="divide-y divide-border rounded-2xl bg-muted/70 text-sm">
        {rows.map((r) => (
          <li key={r.id} className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 px-4 py-3">
            <div className="min-w-0">
              <p className="font-medium">{r.label}</p>
              <p className="text-xs text-muted-foreground" dir="ltr">
                <span className={r.installed ? "text-emerald-600" : tone(r.status ?? "pending")}>
                  {r.installed ? r.version ?? (isAr ? "مثبّت" : "installed") : isAr ? "غير مثبّت" : "not installed"}
                </span>
                {r.status && ` · ${isAr ? statusWord[r.status].ar : statusWord[r.status].en}`}
                {r.needs_key && ` · ${isAr ? "يحتاج مفتاحاً" : "needs key"}`}
              </p>
            </div>
            <div className="flex gap-4">
              {!r.installed && r.can_fix && (
                <button className={link} disabled={busy !== null} onClick={() => onAct(r, "fix")}>
                  {busy === `${r.id}:fix` ? "…" : isAr ? "ثبّت" : "Install"}
                </button>
              )}
              {r.can_update && (
                <button className={link} disabled={busy !== null} onClick={() => onAct(r, "update")}>
                  {busy === `${r.id}:update` ? "…" : isAr ? "حدّث" : "Update"}
                </button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}
