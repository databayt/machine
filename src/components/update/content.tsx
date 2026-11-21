"use client"

import { useCallback, useEffect, useState } from "react"
import { invoke, isTauri } from "@tauri-apps/api/core"

import type { Locale } from "@/components/local/config"

import type { HealthReport } from "../network/types"
import { PageShell, Section } from "../shell/page"
import { OpenInMachine, useInMachine } from "../shell/content"
import { updateCheck, updateInstall, type UpdateInfo } from "./actions"

const DAY_MS = 24 * 60 * 60 * 1000
const link = "text-sm font-medium text-primary rounded-full px-2.5 py-1 transition-colors hover:bg-background disabled:opacity-50"

/** Checks for a new version now and every 24 hours while the app is open. */
export function useUpdateCheck() {
  const [update, setUpdate] = useState<UpdateInfo | null>(null)
  const [checked, setChecked] = useState<Date | null>(null)
  const [error, setError] = useState<string | null>(null)
  const check = useCallback(async () => {
    try {
      setUpdate(await updateCheck())
      setChecked(new Date())
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }, [])
  useEffect(() => {
    if (!isTauri()) return
    void check()
    const id = setInterval(check, DAY_MS)
    return () => clearInterval(id)
  }, [check])
  return { update, checked, error, check }
}

/** The Health & updates tab: this app's version, the health pass, and the self-update. */
export default function HealthContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (en: string, ar: string) => (isAr ? ar : en)
  const inApp = useInMachine()
  const { update, checked, error, check } = useUpdateCheck()
  const [report, setReport] = useState<HealthReport | null>(null)
  const [installing, setInstalling] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (inApp) invoke<HealthReport | null>("plugin:network|health").then(setReport).catch(() => null)
  }, [inApp])

  async function runCheck() {
    setBusy(true)
    try {
      setReport(await invoke<HealthReport>("plugin:network|check_now"))
    } catch (e) {
      setFailure(String(e))
    } finally {
      setBusy(false)
    }
  }

  async function install() {
    setInstalling(true)
    setFailure(null)
    try {
      await updateInstall()
    } catch (e) {
      setFailure(String(e))
      setInstalling(false)
    }
  }

  if (inApp === null) return null
  if (!inApp)
    return (
      <PageShell title={t("Health & updates", "الصحة والتحديثات")}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  return (
    <PageShell title={t("Health & updates", "الصحة والتحديثات")}>
      <Section title={t("This app", "هذا التطبيق")}>
      <div className="space-y-2 rounded-2xl bg-background p-4">
        {update ? (
          <div className="space-y-2 text-sm">
            <p>
              {t("Version", "الإصدار")} <span dir="ltr" className="font-medium">{update.version}</span> {t("is ready; you have", "جاهز؛ لديك")} <span dir="ltr">{update.current}</span>.
            </p>
            {update.notes && <p className="whitespace-pre-wrap text-xs text-muted-foreground">{update.notes}</p>}
            <button className="inline-flex h-9 items-center rounded-full bg-foreground px-4 text-sm font-medium text-background disabled:opacity-50" disabled={installing} onClick={install}>
              {installing ? t("Installing…", "جارٍ التثبيت…") : t("Install and restart", "ثبّت وأعد التشغيل")}
            </button>
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            {checked ? t("Up to date.", "محدَّث.") : t("Checking…", "جارٍ الفحص…")}
            {error && <span className="text-destructive"> {error}</span>}
          </p>
        )}
        {failure && <p className="text-sm text-destructive">{failure}</p>}
        <button className={link} onClick={check}>{t("Check for updates", "ابحث عن تحديثات")}</button>
      </div>
      </Section>

      <Section
        title={t("Health", "الصحة")}
        action={<button className={link} disabled={busy} onClick={runCheck}>{busy ? "…" : t("Check now", "افحص الآن")}</button>}
      >
      <div className="rounded-2xl bg-background p-4">
        {report ? (
          <ul className="divide-y divide-border text-sm">
            {report.items.map((i) => (
              <li key={i.key} className="flex items-start gap-3 py-2">
                <span className={i.ok ? "text-emerald-600" : "text-destructive"} aria-hidden>{i.ok ? "✓" : "✗"}</span>
                <span className="min-w-0"><span className="font-medium" dir="ltr">{i.key}</span> <span className="text-xs text-muted-foreground" dir="ltr">{i.detail}</span></span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-muted-foreground">{t("No health pass has finished yet.", "لم ينتهِ أي فحص صحة بعد.")}</p>
        )}
      </div>
      </Section>
    </PageShell>
  )
}
