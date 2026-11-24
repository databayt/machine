"use client"

import { ArrowLeft, ChevronRight, Loader2 } from "lucide-react"
import { useCallback, useEffect, useState } from "react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"

import type { Copy } from "../chat/config"
import { providersOpen, providersSave, providersSetup, providersStatus, providersTest } from "./actions"
import { providersCopy as c } from "./config"
import type { ProviderRow, ProvidersView, TestResult } from "./types"

const pill = "text-sm font-medium rounded-full px-3 py-1.5 transition-colors disabled:opacity-50"
const field = "bg-muted h-10 min-w-0 flex-1 rounded-full px-4 text-sm outline-none"

type T = (c: Copy) => string

function word<K extends string>(table: Record<K, Copy>, key: string, t: T): string {
  return key in table ? t(table[key as K]) : key
}

const dot = (status: string) =>
  status === "connected" || status === "direct-ok"
    ? "bg-emerald-500"
    : status === "exhausted"
      ? "bg-amber-500"
      : status === "error" || status === "direct-down"
        ? "bg-destructive"
        : "bg-muted-foreground/40"

function fill(text: string, values: Record<string, string>): string {
  return Object.entries(values).reduce((out, [k, v]) => out.replace(`{${k}}`, v), text)
}

/** A reset time that is an ISO string or epoch seconds, in the person's language; "" when unreadable. */
function when(raw: string, lang: Locale): string {
  const ms = /^\d+$/.test(raw) ? Number(raw) * (raw.length > 11 ? 1 : 1000) : Date.parse(raw)
  if (!Number.isFinite(ms)) return ""
  return new Date(ms).toLocaleString(lang === "ar" ? "ar" : "en", { dateStyle: "medium", timeStyle: "short" })
}

/** The one line of numbers under a row: requests, success rate, quota. */
function figures(r: ProviderRow, t: T, lang: Locale): string[] {
  const nf = new Intl.NumberFormat(lang === "ar" ? "ar" : "en", { maximumFractionDigits: 1 })
  const n = (v: number) => nf.format(v)
  const out: string[] = []
  if (r.requests != null) out.push(fill(t(c.requests), { n: n(r.requests) }))
  if (r.success_pct != null) out.push(fill(t(c.success), { n: n(r.success_pct) }))
  if (r.avg_ms != null) out.push(fill(t(c.avg), { n: n(Math.round(r.avg_ms)) }))
  const q = r.quota
  if (q) {
    const unit = q.unit ? ` ${q.unit}` : ""
    if (q.remaining != null) out.push(fill(t(c.left), { n: `${n(q.remaining)}${unit}` }))
    else if (q.used != null && q.limit != null) out.push(fill(t(c.used), { used: n(q.used), limit: `${n(q.limit)}${unit}` }))
    const at = q.resets_at ? when(q.resets_at, lang) : ""
    if (at) out.push(fill(t(c.resets), { when: at }))
  }
  return out
}

/**
 * Settings → Model → Providers: one row per source in kun's catalog, merged with its harvest ledger.
 * A list that swaps for the row's own screen: nothing scrolls past one list, nothing is a link.
 */
export function Providers({ lang, t, onBack }: { lang: Locale; t: T; onBack: () => void }) {
  const [view, setView] = useState<ProvidersView | null>(null)
  const [open, setOpen] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const reload = useCallback(async () => {
    const v = await providersStatus()
    if (v) setView(v)
  }, [])

  useEffect(() => {
    reload().catch((e) => setError(String(e)))
  }, [reload])

  async function setup() {
    setBusy(true)
    setError(null)
    try {
      setView(await providersSetup())
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  const row = view?.rows.find((r) => r.id === open)
  if (view && row) {
    return (
      <Detail
        key={row.id}
        row={row}
        lang={lang}
        t={t}
        onBack={() => setOpen(null)}
        onView={setView}
      />
    )
  }

  const anyReady = view?.combos.some((x) => x.ready) ?? false
  const needsSetup = !!view && (view.gateway !== "ok" || !anyReady)
  const headline = !view
    ? t(c.loading)
    : view.gateway === "missing"
      ? t(c.gatewayMissing)
      : view.gateway === "down"
        ? t(c.gatewayDown)
        : view.pool_ready
          ? t(c.poolReady)
          : t(c.poolNotReady)

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <BackButton t={t} onClick={onBack} />
      <div className="flex flex-wrap items-center gap-3">
        <span className="flex items-center gap-2 text-sm font-medium">
          <span className={cn("size-2 rounded-full", view?.gateway === "ok" && view.pool_ready ? "bg-emerald-500" : "bg-amber-500")} aria-hidden />
          {headline}
        </span>
        {view?.stale && <span className="text-muted-foreground text-xs">{t(c.stale)}</span>}
        {needsSetup && view?.scripts_ready && view.gateway !== "missing" && (
          <button type="button" onClick={() => void setup()} disabled={busy} className={cn(pill, "bg-foreground text-background hover:bg-foreground/90")}>
            {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : t(c.setUp)}
          </button>
        )}
      </div>
      {view && !view.scripts_ready && <p className="text-muted-foreground text-sm">{t(c.noScripts)}</p>}
      {view?.error && <p className="text-destructive text-sm">{view.error}</p>}
      {error && <p className="text-destructive text-sm">{error}</p>}
      {view && view.combos.length > 0 && (
        <div className="flex flex-wrap gap-1.5" dir="ltr">
          {view.combos.map((x) => (
            <span key={x.name} className="bg-muted text-muted-foreground flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs">
              <span className={cn("size-1.5 rounded-full", x.ready ? "bg-emerald-500" : "bg-muted-foreground/40")} aria-hidden />
              {x.name}
            </span>
          ))}
        </div>
      )}
      {view && view.rows.length === 0 && !view.error && <p className="text-muted-foreground text-sm">{t(c.empty)}</p>}
      <ul className="flex flex-col gap-2">
        {view?.rows.map((r) => (
          <li key={r.id}>
            <button
              type="button"
              onClick={() => setOpen(r.id)}
              className="bg-background hover:bg-muted/50 flex w-full min-w-0 items-center gap-3 rounded-2xl p-4 text-start transition-colors"
            >
              <span className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-semibold">
                    <bdi>{r.name}</bdi>
                  </span>
                  <Tier row={r} t={t} />
                </span>
                <span className="text-muted-foreground flex items-center gap-1.5 text-sm">
                  <span className={cn("size-1.5 shrink-0 rounded-full", dot(r.status))} aria-hidden />
                  {word(c.status, r.status, t)}
                  {figures(r, t, lang).slice(0, 2).map((f) => (
                    <span key={f} className="truncate">
                      · {f}
                    </span>
                  ))}
                </span>
              </span>
              <ChevronRight className="text-muted-foreground size-4 shrink-0 rtl:rotate-180" aria-hidden />
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

function BackButton({ t, onClick }: { t: T; onClick: () => void }) {
  return (
    <button type="button" onClick={onClick} aria-label={t(c.back)} className="bg-muted hover:bg-muted/70 flex size-9 items-center justify-center self-start rounded-full transition-colors">
      <ArrowLeft className="size-4 rtl:rotate-180" aria-hidden />
    </button>
  )
}

function Tier({ row, t }: { row: ProviderRow; t: T }) {
  return <span className="bg-muted text-muted-foreground shrink-0 rounded-full px-2 py-0.5 text-xs">{word(c.tier, row.tier, t)}</span>
}

/** One provider's own screen: its numbers, its note, and the actions its kind allows. */
function Detail({
  row,
  lang,
  t,
  onBack,
  onView,
}: {
  row: ProviderRow
  lang: Locale
  t: T
  onBack: () => void
  onView: (v: ProvidersView) => void
}) {
  const [key, setKey] = useState("")
  const [account, setAccount] = useState("")
  const [busy, setBusy] = useState<"save" | "test" | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<TestResult | null>(null)

  async function save() {
    // The key leaves the field the moment it is sent; it is never held or shown again.
    const k = key
    const a = account
    setKey("")
    setAccount("")
    setBusy("save")
    setError(null)
    setResult(null)
    try {
      onView(await providersSave(row.id, k, a))
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  async function test() {
    setBusy("test")
    setError(null)
    setResult(null)
    try {
      setResult(await providersTest(row.id))
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  async function openSignup() {
    setError(null)
    try {
      await providersOpen(row.id)
    } catch (e) {
      setError(String(e))
    }
  }

  const lines = figures(row, t, lang)
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <BackButton t={t} onClick={onBack} />
      <div className="bg-background flex min-w-0 flex-col gap-3 rounded-2xl p-4">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate font-semibold">
            <bdi>{row.name}</bdi>
          </span>
          <Tier row={row} t={t} />
        </div>
        <span className="text-muted-foreground flex items-center gap-1.5 text-sm">
          <span className={cn("size-1.5 shrink-0 rounded-full", dot(row.status))} aria-hidden />
          {word(c.status, row.status, t)}
        </span>
        {lines.length > 0 && (
          <ul className="text-muted-foreground flex flex-col gap-0.5 text-sm">
            {lines.map((l) => (
              <li key={l}>{l}</li>
            ))}
          </ul>
        )}
        {row.note && <p className="text-muted-foreground text-sm">{row.note}</p>}

        {row.mode === "paid" && (
          <p className="text-sm">
            {t(c.paidNote)}
            {row.decide && <span className="text-muted-foreground block text-xs">{row.decide}</span>}
          </p>
        )}
        {row.mode === "plan" && <p className="text-sm">{t(c.planNote)}</p>}

        {row.mode === "pool" && (
          <form
            className="flex flex-col gap-2"
            onSubmit={(e) => {
              e.preventDefault()
              void save()
            }}
          >
            <input
              type="password"
              autoComplete="off"
              spellCheck={false}
              dir="ltr"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder={row.key_set ? t(c.keyReplace) : t(c.keyPlaceholder)}
              aria-label={t(c.keyPlaceholder)}
              className={field}
            />
            {row.needs_account && (
              <input
                type="text"
                autoComplete="off"
                spellCheck={false}
                dir="ltr"
                value={account}
                onChange={(e) => setAccount(e.target.value)}
                placeholder={t(c.accountPlaceholder)}
                aria-label={t(c.accountPlaceholder)}
                className={field}
              />
            )}
            <button type="submit" disabled={!key.trim() || busy !== null} className={cn(pill, "bg-foreground text-background hover:bg-foreground/90 self-start")}>
              {busy === "save" ? <Loader2 className="size-4 animate-spin" aria-hidden /> : t(c.save)}
            </button>
          </form>
        )}

        {error && <p className="text-destructive text-sm">{error}</p>}
        {result && (
          <p className={cn("text-sm", result.ok ? "text-emerald-600 dark:text-emerald-400" : "text-destructive")}>
            {result.ok ? t(c.testOk) : t(c.testFail)}
            {result.latency_ms != null && <span> · {Math.round(result.latency_ms)} ms</span>}
            {result.message && <span> · {result.message}</span>}
          </p>
        )}

        {(row.mode === "pool" || row.mode === "direct") && (
          <div className="flex flex-wrap items-center gap-2">
            {row.signup && (
              <button type="button" onClick={() => void openSignup()} className={cn(pill, "bg-muted hover:bg-muted/70")}>
                {row.mode === "pool" ? t(c.getKey) : t(c.signUp)}
              </button>
            )}
            {row.can_test && (
              <button type="button" onClick={() => void test()} disabled={busy !== null} className={cn(pill, "bg-muted hover:bg-muted/70")}>
                {busy === "test" ? <Loader2 className="size-4 animate-spin" aria-hidden /> : t(c.test)}
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
