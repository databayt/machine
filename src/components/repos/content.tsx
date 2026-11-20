"use client"

import { useCallback, useEffect, useState } from "react"

import type { Locale } from "@/components/local/config"

import { PageShell, Section } from "../shell/page"
import { OpenInMachine, useInMachine } from "../shell/content"
import {
  cloneRepo,
  failureOf,
  getRoot,
  moveBlockers,
  listRepos,
  openRemote,
  pullRepo,
  removeRepo,
  setMode,
  setRoot,
  setRemote,
  type RepoRow,
  type RootInfo,
} from "./actions"
import { copy, lossesText } from "./config"

const link = "text-sm font-medium text-primary underline-offset-4 hover:underline disabled:opacity-50"

export default function ReposContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const inApp = useInMachine()
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const [rows, setRows] = useState<RepoRow[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  const reload = useCallback(() => {
    listRepos()
      .then((r) => {
        setRows(r)
        setError(null)
      })
      .catch((e) => setError(failureOf(e).message))
  }, [])

  useEffect(() => {
    if (inApp) reload()
  }, [inApp, reload])

  if (inApp === false)
    return (
      <PageShell title={isAr ? "المستودعات" : "Repositories"}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  return (
    <PageShell title={isAr ? "المستودعات" : "Repositories"} description={t(copy.subtitle)}>
      <RootPicker isAr={isAr} onChanged={reload} />
      {error && <p className="text-sm text-destructive">{error}</p>}
      {!rows && !error && <p className="text-muted-foreground">{t(copy.loading)}</p>}
      {rows && rows.length === 0 && <p className="text-muted-foreground">{t(copy.empty)}</p>}
      {rows && rows.length > 0 && (
        <Section>
          <ul className="flex flex-col gap-2">
            {rows.map((r) => (
              <Row key={r.name} row={r} isAr={isAr} onChanged={reload} />
            ))}
          </ul>
        </Section>
      )}
    </PageShell>
  )
}

function Row({ row, isAr, onChanged }: { row: RepoRow; isAr: boolean; onChanged: () => void }) {
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const [busy, setBusy] = useState(false)
  const [progress, setProgress] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [losses, setLosses] = useState<string[]>([])
  const [confirming, setConfirming] = useState(false)
  const [editing, setEditing] = useState(false)
  const [url, setUrl] = useState("")

  async function run(fn: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    setLosses([])
    try {
      await fn()
      onChanged()
    } catch (e) {
      const f = failureOf(e)
      if (f.code === "refused") setLosses(lossesText(f.losses, isAr))
      else setError(f.message)
    } finally {
      setBusy(false)
      setProgress(null)
      setConfirming(false)
    }
  }

  const meta = row.local
    ? [
        row.branch,
        row.behind > 0 ? `↓${row.behind}` : null,
        row.ahead > 0 ? `↑${row.ahead}` : null,
        row.dirty ? t(copy.dirty) : t(copy.clean),
        row.mode === "sprint" ? t(copy.sprint) : t(copy.keep),
        row.private ? t(copy.private) : null,
      ]
    : [t(copy.notCloned), row.private ? t(copy.private) : null]

  return (
    <li className="rounded-2xl bg-background px-5 py-4">
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h2 className="text-xl font-bold tracking-tight">{row.name}</h2>
        <p className="text-sm text-muted-foreground">{meta.filter(Boolean).join(" · ")}</p>
      </div>
      {row.description && <p className="mt-1 text-sm text-muted-foreground">{row.description}</p>}

      <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1">
        {!row.local ? (
          <>
            <button className={link} disabled={busy} onClick={() => run(() => cloneRepo(row.name, undefined, (e) => setProgress(e.message)))}>
              {busy ? t(copy.working) : t(copy.clone)}
            </button>
            <button className={link} disabled={busy} onClick={() => run(() => cloneRepo(row.name, 0, (e) => setProgress(e.message)))}>
              {t(copy.cloneFull)}
            </button>
          </>
        ) : (
          <>
            <button className={link} disabled={busy} onClick={() => run(() => pullRepo(row.name, (e) => setProgress(e.message)))}>
              {busy ? t(copy.working) : t(copy.pull)}
            </button>
            <button
              className={link}
              disabled={busy}
              onClick={() => run(() => (row.mode === "sprint" ? setMode(row.name, "keep") : setMode(row.name, "sprint", prompt(t(copy.sprintName)) ?? undefined)))}
            >
              {row.mode === "sprint" ? t(copy.keep) : t(copy.sprint)}
            </button>
            <button className={link} disabled={busy} onClick={() => setEditing((v) => !v)}>
              {t(copy.remote)}
            </button>
            {confirming ? (
              <span className="inline-flex items-center gap-3 text-sm">
                <span className="text-muted-foreground">{t(copy.confirmRemove)}</span>
                <button className="font-medium text-destructive underline-offset-4 hover:underline" disabled={busy} onClick={() => run(() => removeRepo(row.name))}>
                  {t(copy.yes)}
                </button>
                <button className={link} onClick={() => setConfirming(false)}>
                  {t(copy.cancel)}
                </button>
              </span>
            ) : (
              <button className="text-sm font-medium text-destructive underline-offset-4 hover:underline disabled:opacity-50" disabled={busy} onClick={() => setConfirming(true)}>
                {t(copy.remove)}
              </button>
            )}
          </>
        )}
        <button className={link} onClick={() => openRemote(row.name)}>
          {t(copy.open)}
        </button>
      </div>

      {editing && (
        <form
          className="mt-3 flex items-center gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            run(() => setRemote(row.name, url.trim())).then(() => setEditing(false))
          }}
        >
          <input
            dir="ltr"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={t(copy.remoteHint)}
            aria-label={t(copy.remote)}
            className="h-9 min-w-0 flex-1 rounded-full bg-muted px-4 text-sm outline-none"
          />
          <button type="submit" className={link} disabled={busy || !url.trim()}>
            {t(copy.save)}
          </button>
        </form>
      )}

      {progress && <p className="mt-2 text-xs text-muted-foreground" dir="ltr">{progress}</p>}
      {error && <p className="mt-2 text-sm text-destructive">{error}</p>}
      {losses.length > 0 && (
        <div className="mt-2 text-sm text-destructive">
          <p className="font-medium">{t(copy.refused)}</p>
          <ul className="list-disc ps-5">
            {losses.map((l) => (
              <li key={l}>{l}</li>
            ))}
          </ul>
        </div>
      )}
    </li>
  )
}

const rootCopy = {
  where: { en: "Where the repos live", ar: "مكان المستودعات" },
  source: { setting: { en: "chosen here", ar: "اخترته هنا" }, installer: { en: "from the installer", ar: "من المثبّت" }, default: { en: "default", ar: "الافتراضي" } },
  clones: { en: "clones here", ar: "نسخة هنا" },
  change: { en: "Change folder", ar: "غيّر المجلد" },
  move: { en: "Move the clones", ar: "انقل النسخ" },
  leave: { en: "Leave them in place", ar: "اتركها مكانها" },
  hint: { en: "Absolute path, e.g. /Users/you/code", ar: "مسار مطلق مثل /Users/you/code" },
  blocked: { en: "Not moved: these clones have unsaved or unpushed work, or the name is taken there:", ar: "لم يُنقل: هذه النسخ فيها عمل غير محفوظ أو غير مدفوع، أو الاسم موجود هناك:" },
}

/** The repos root: shown, and changed with "move the clones" (refused when work would be lost) or "leave them". */
function RootPicker({ isAr, onChanged }: { isAr: boolean; onChanged: () => void }) {
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const [info, setInfo] = useState<RootInfo | null>(null)
  const [path, setPath] = useState("")
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [blocked, setBlocked] = useState<string[]>([])

  useEffect(() => {
    getRoot().then((r) => {
      setInfo(r)
      if (r) setPath(r.root)
    }).catch((e) => setError(failureOf(e).message))
  }, [])

  async function apply(moveClones: boolean) {
    setBusy(true)
    setError(null)
    setBlocked([])
    try {
      if (moveClones) {
        const b = await moveBlockers(path.trim())
        if (b.length > 0) {
          setBlocked(b.map((x) => `${x.name}${x.losses.length ? ` (${lossesText(x.losses, isAr).join(", ")})` : ""}`))
          return
        }
      }
      setInfo(await setRoot(path.trim(), moveClones))
      setOpen(false)
      onChanged()
    } catch (e) {
      const f = failureOf(e)
      setError(f.message)
    } finally {
      setBusy(false)
    }
  }

  if (!info) return error ? <p className="text-sm text-destructive">{error}</p> : null
  return (
    <Section title={t(rootCopy.where)} action={<p className="text-xs text-muted-foreground">{t(rootCopy.source[info.source])} · {info.clones} {t(rootCopy.clones)}</p>}>
    <div className="rounded-2xl bg-background p-4">
      <p className="mt-1 break-all text-sm" dir="ltr">{info.root}</p>
      <button className={`${link} mt-2`} onClick={() => setOpen((v) => !v)}>{t(rootCopy.change)}</button>
      {open && (
        <div className="mt-2 space-y-2">
          <input dir="ltr" value={path} onChange={(e) => setPath(e.target.value)} placeholder={t(rootCopy.hint)} aria-label={t(rootCopy.where)} className="h-9 w-full rounded-full bg-muted px-4 text-sm outline-none" />
          <div className="flex flex-wrap gap-4">
            <button className={link} disabled={busy || !path.trim() || path.trim() === info.root} onClick={() => apply(true)}>{t(rootCopy.move)}</button>
            <button className={link} disabled={busy || !path.trim() || path.trim() === info.root} onClick={() => apply(false)}>{t(rootCopy.leave)}</button>
          </div>
        </div>
      )}
      {blocked.length > 0 && (
        <div className="mt-2 text-sm text-destructive">
          <p className="font-medium">{t(rootCopy.blocked)}</p>
          <ul className="list-disc ps-5">{blocked.map((b) => <li key={b}>{b}</li>)}</ul>
        </div>
      )}
      {error && <p className="mt-2 text-sm text-destructive">{error}</p>}
    </div>
    </Section>
  )
}
