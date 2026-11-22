"use client"

import { useCallback, useEffect, useState } from "react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"

import { tr } from "../chat/config"
import { readinessFix } from "../readiness/actions"
import { OpenInMachine, useInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { launchersApply, launchersStatus, type LauncherRow, type LaunchersView } from "./actions"
import { copy } from "./config"

const pill = "rounded-full px-3 py-1.5 text-sm font-medium text-primary transition-colors hover:bg-muted disabled:opacity-50"

/** The ids to write after flipping one launcher; catalog order is the writer's, not ours. */
export function toggled(rows: Pick<LauncherRow, "id" | "enabled">[], id: string): string[] {
  return rows.filter((r) => (r.id === id ? !r.enabled : r.enabled)).map((r) => r.id)
}

/** Settings → Agents: one row per launcher, with a switch that writes the managed block in the shell. */
export default function LaunchersContent({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const [view, setView] = useState<LaunchersView | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  const reload = useCallback(async () => {
    const v = await launchersStatus()
    if (v) setView(v)
  }, [])

  useEffect(() => {
    if (!inApp) return
    reload().catch((e) => setMessage(String(e)))
  }, [inApp, reload])

  async function flip(id: string) {
    if (!view) return
    setBusy(id)
    setMessage(null)
    setSaved(false)
    try {
      setView(await launchersApply(toggled(view.rows, id)))
      setSaved(true)
    } catch (e) {
      setMessage(String(e))
    } finally {
      setBusy(null)
    }
  }

  async function install(row: LauncherRow) {
    if (!row.needs) return
    setBusy(`${row.id}:install`)
    setMessage(null)
    try {
      const out = await readinessFix(row.needs)
      if (out.message) setMessage(out.message)
      await reload()
    } catch (e) {
      setMessage(String(e))
    } finally {
      setBusy(null)
    }
  }

  if (inApp === null) return null
  return (
    <PageShell title={t(copy.title)}>
      {!inApp ? (
        <OpenInMachine lang={lang} />
      ) : (
        <>
          <p className="max-w-xl text-sm text-muted-foreground">{t(copy.intro)}</p>
          {message && <p className="whitespace-pre-wrap text-sm text-destructive">{message}</p>}
          {view && !view.found ? (
            <p className="rounded-2xl bg-background p-4 text-sm text-muted-foreground">{t(copy.noFile)}</p>
          ) : (
            view && (
              <>
                <ul className="divide-y divide-border rounded-2xl bg-background text-sm">
                  {view.rows.map((r) => (
                    <li key={r.id} className="flex items-center gap-4 px-4 py-3">
                      <span
                        dir="ltr"
                        className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-muted font-mono text-lg font-semibold"
                      >
                        {r.id}
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="truncate font-medium" dir="ltr">
                          {r.command}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {r.bypass ? (r.bypassNote ? t(copy.bypassConfig) : t(copy.bypass)) : t(copy.noBypass)}
                          {" · "}
                          <span className={r.installed ? "text-emerald-600" : "text-amber-600"}>
                            {r.installed ? t(copy.installed) : t(copy.notInstalled)}
                          </span>
                        </p>
                      </div>
                      {!r.installed && r.needs && (
                        <button type="button" className={pill} disabled={busy !== null} onClick={() => void install(r)}>
                          {busy === `${r.id}:install` ? t(copy.installing) : t(copy.install)}
                        </button>
                      )}
                      <button
                        type="button"
                        role="switch"
                        aria-checked={r.enabled}
                        aria-label={`${r.id} · ${r.enabled ? t(copy.on) : t(copy.off)}`}
                        disabled={busy !== null}
                        onClick={() => void flip(r.id)}
                        className={cn(
                          "relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-50",
                          r.enabled ? "bg-foreground" : "bg-muted",
                        )}
                      >
                        <span
                          className={cn(
                            "absolute top-0.5 size-5 rounded-full bg-background transition-all",
                            r.enabled ? "start-[1.375rem]" : "start-0.5",
                          )}
                        />
                      </button>
                    </li>
                  ))}
                </ul>
                <p className="text-xs text-muted-foreground">
                  {t(copy.newTerminal)}
                  {saved && ` ${t(copy.saved)}`}
                </p>
                {view.files.length > 0 && (
                  <p className="text-xs text-muted-foreground" dir="ltr">
                    {t(copy.files)}: {view.files.join(", ")}
                  </p>
                )}
              </>
            )
          )}
        </>
      )}
    </PageShell>
  )
}
