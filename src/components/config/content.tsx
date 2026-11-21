"use client"

import { useCallback, useEffect, useState } from "react"

import type { Locale } from "@/components/local/config"

import { departments as listDepartments } from "../pack/actions"
import { PageShell, Section } from "../shell/page"
import { OpenInMachine, useInMachine } from "../shell/content"
import { loadSettings, saveSettings } from "../settings/actions"
import type { Settings } from "../settings/types"
import {
  configLoad,
  loadSecrets,
  mcpAdd,
  mcpList,
  mcpRemove,
  removeSetting,
  setNotify,
  setSetting,
  type ConfigView,
  type McpView,
  type WriteOutcome,
} from "./actions"

const link = "text-sm font-medium text-primary rounded-full px-2.5 py-1 transition-colors hover:bg-background disabled:opacity-50"
const field = "h-10 min-w-0 flex-1 rounded-full bg-muted px-4 text-sm outline-none"
const card = "space-y-3 rounded-2xl bg-background p-4"

/** The Configuration tab: department, notify line, personal settings keys, MCP servers, secrets. */
export default function ConfigContent({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = (en: string, ar: string) => (isAr ? ar : en)
  const inApp = useInMachine()
  const [view, setView] = useState<ConfigView | null>(null)
  const [mcp, setMcp] = useState<McpView | null>(null)
  const [mcpBusy, setMcpBusy] = useState(false)
  const [settings, setSettings] = useState<Settings | null>(null)
  const [departments, setDepartments] = useState<string[]>([])
  const [notify, setNotifyText] = useState("")
  const [gist, setGist] = useState("")
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)
  const [busy, setBusy] = useState(false)

  const reload = useCallback(async () => {
    const v = await configLoad()
    if (!v) return
    setView(v)
    setNotifyText(v.notify ?? "")
  }, [])

  const loadMcp = useCallback(() => {
    setMcpBusy(true)
    mcpList()
      .then(setMcp)
      .catch((e) => setMessage({ ok: false, text: String(e) }))
      .finally(() => setMcpBusy(false))
  }, [])

  useEffect(() => {
    if (!inApp) return
    reload().catch((e) => setMessage({ ok: false, text: String(e) }))
    loadSettings().then(setSettings).catch(() => null)
    listDepartments().then(setDepartments).catch(() => null)
    loadMcp()
  }, [inApp, reload, loadMcp])

  /** Runs a guarded write and says what happened, including a restore after a RED health check. */
  async function write(job: () => Promise<WriteOutcome>, after?: () => void) {
    setBusy(true)
    setMessage(null)
    try {
      const out = await job()
      setMessage({
        ok: out.ok,
        text: out.rolled_back
          ? t("Health went RED, so the previous content was restored.", "تحول الفحص إلى الأحمر فأُعيد المحتوى السابق.")
          : `${t("Saved", "تم الحفظ")} · health ${out.verdict}`,
      })
      await reload()
      after?.()
    } catch (e) {
      setMessage({ ok: false, text: String(e) })
    } finally {
      setBusy(false)
    }
  }

  if (inApp === null) return null
  if (!inApp)
    return (
      <PageShell title={t("Configuration", "التهيئة")}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )

  return (
    <PageShell title={t("Configuration", "التهيئة")}>
      {message && <p className={`text-sm ${message.ok ? "text-muted-foreground" : "text-destructive"}`}>{message.text}</p>}

      {settings && (
        <Section title={t("Department", "القسم")}>
        <div className={card}>
          <select
            value={settings.department}
            onChange={(e) => saveSettings({ ...settings, department: e.target.value }).then(setSettings).catch((er) => setMessage({ ok: false, text: String(er) }))}
            className="h-10 w-full rounded-full bg-muted px-4 text-sm"
          >
            {(departments.includes(settings.department) ? departments : [settings.department, ...departments]).map((d) => (
              <option key={d} value={d}>{d}</option>
            ))}
          </select>
        </div>
        </Section>
      )}

      <Section title={t("Notifications line", "سطر الإشعارات")}>
      <div className={card}>
        <p className="text-xs text-muted-foreground" dir="ltr">~/.claude/.kun-notify</p>
        <form
          className="flex items-center gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            void write(() => setNotify(notify))
          }}
        >
          <input dir="ltr" className={field} value={notify} maxLength={200} onChange={(e) => setNotifyText(e.target.value)} />
          <button type="submit" className={link} disabled={busy || !notify.trim()}>{t("Save", "حفظ")}</button>
        </form>
      </div>
      </Section>

      <Section title={t("Personal settings", "إعدادات شخصية")}>
      <div className={card}>
        <p className="text-xs text-muted-foreground" dir="ltr">~/.claude/settings.json</p>
        <p className="text-xs text-muted-foreground">
          {view?.template_found
            ? t("Only model, voice and keys kun does not manage.", "النموذج والصوت والمفاتيح التي لا يديرها kun فقط.")
            : t("kun's settings template was not found, so only model and voice can be edited.", "لم يُعثر على قالب إعدادات kun، فيُعدَّل النموذج والصوت فقط.")}
        </p>
        <ul className="space-y-2">
          {view?.settings.map((s) => (
            <SettingRow key={s.key} entry={s} busy={busy} isAr={isAr} onSave={(v) => write(() => setSetting(s.key, v))} onRemove={() => write(() => removeSetting(s.key))} />
          ))}
        </ul>
        <NewSetting busy={busy} isAr={isAr} onAdd={(k, v) => write(() => setSetting(k, v))} />
      </div>
      </Section>

      <Section
        title={t("MCP servers", "خوادم MCP")}
        action={<button className={link} disabled={mcpBusy} onClick={loadMcp}>{mcpBusy ? "…" : t("Refresh", "حدّث")}</button>}
      >
      <div className={card}>
        <ul className="divide-y divide-border text-sm">
          {mcp?.installed.map((s) => (
            <li key={s.name} className="flex items-center justify-between gap-3 py-2">
              <div className="min-w-0">
                <p className="font-medium" dir="ltr">{s.name}</p>
                <p className="truncate text-xs text-muted-foreground" dir="ltr">{s.connected === true ? "✓ " : s.connected === false ? "✗ " : ""}{s.summary}</p>
              </div>
              <button className="text-sm font-medium text-destructive rounded-full px-2.5 py-1 transition-colors hover:bg-background disabled:opacity-50" disabled={busy} onClick={() => write(() => mcpRemove(s.name), loadMcp)}>
                {t("Remove", "أزل")}
              </button>
            </li>
          ))}
          {mcp?.available.map((s) => (
            <li key={s.name} className="flex items-center justify-between gap-3 py-2">
              <div className="min-w-0">
                <p className="font-medium" dir="ltr">{s.name} <span className="text-xs font-normal text-muted-foreground">· {t("not installed", "غير مثبّت")}{s.skipped ? ` · ${t("setup skips it", "الإعداد يتخطاه")}` : ""}</span></p>
                {s.description && <p className="truncate text-xs text-muted-foreground">{s.description}</p>}
              </div>
              <button className={link} disabled={busy} onClick={() => write(() => mcpAdd(s.name), loadMcp)}>{t("Add", "أضف")}</button>
            </li>
          ))}
        </ul>
      </div>
      </Section>

      <Section title={t("Team secrets", "أسرار الفريق")}>
      <div className={card}>
        <p className="text-sm">
          <span dir="ltr">~/.claude/.env</span> · {view?.env_present ? t("present", "موجود") : t("absent", "غير موجود")}
          <span className="block text-xs text-muted-foreground">{t("Values are never shown here.", "لا تُعرض القيم هنا أبداً.")}</span>
        </p>
        <form
          className="flex items-center gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            loadSecrets(gist.trim())
              .then(() => setMessage({ ok: true, text: t("Opened Terminal. Finish there.", "فُتحت الطرفية. أكمل هناك.") }))
              .catch((er) => setMessage({ ok: false, text: String(er) }))
          }}
        >
          <input dir="ltr" className={field} value={gist} onChange={(e) => setGist(e.target.value)} placeholder="GIST_ID" aria-label="Gist id" />
          <button type="submit" className={link} disabled={!gist.trim()}>{t("Load secrets from Gist", "حمّل الأسرار من Gist")}</button>
        </form>
      </div>
      </Section>
    </PageShell>
  )
}

function SettingRow({ entry, busy, isAr, onSave, onRemove }: { entry: ConfigView["settings"][number]; busy: boolean; isAr: boolean; onSave: (json: string) => void; onRemove: () => void }) {
  const [value, setValue] = useState(entry.value)
  useEffect(() => setValue(entry.value), [entry.value])
  return (
    <li className="flex items-center gap-2">
      <span className="w-36 shrink-0 truncate text-sm font-medium" dir="ltr">{entry.key}</span>
      {entry.hidden ? (
        <span className="text-xs text-muted-foreground">{isAr ? "مخفي" : "hidden"}</span>
      ) : (
        <>
          <input dir="ltr" className={field} value={value} onChange={(e) => setValue(e.target.value)} aria-label={entry.key} />
          <button className={link} disabled={busy || value === entry.value} onClick={() => onSave(value)}>{isAr ? "حفظ" : "Save"}</button>
          <button className="text-sm font-medium text-destructive rounded-full px-2.5 py-1 transition-colors hover:bg-background disabled:opacity-50" disabled={busy} onClick={onRemove}>{isAr ? "أزل" : "Remove"}</button>
        </>
      )}
    </li>
  )
}

function NewSetting({ busy, isAr, onAdd }: { busy: boolean; isAr: boolean; onAdd: (key: string, json: string) => void }) {
  const [key, setKey] = useState("")
  const [value, setValue] = useState("")
  return (
    <form
      className="flex items-center gap-2"
      onSubmit={(e) => {
        e.preventDefault()
        onAdd(key.trim(), value.trim())
        setKey("")
        setValue("")
      }}
    >
      <input dir="ltr" className="h-10 w-36 shrink-0 rounded-full bg-muted px-4 text-sm outline-none" value={key} onChange={(e) => setKey(e.target.value)} placeholder="key" aria-label="key" />
      <input dir="ltr" className={field} value={value} onChange={(e) => setValue(e.target.value)} placeholder='"value" or {"json": true}' aria-label="value" />
      <button type="submit" className={link} disabled={busy || !key.trim() || !value.trim()}>{isAr ? "أضف" : "Add"}</button>
    </form>
  )
}
