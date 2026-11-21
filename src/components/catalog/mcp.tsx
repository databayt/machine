"use client"

import { useCallback, useEffect, useState } from "react"

import type { Locale } from "@/components/local/config"
import { Switch } from "@/components/ui/switch"

import { mcpAdd, mcpList, mcpRemove } from "../config/actions"
import type { McpView, WriteOutcome } from "../config/types"
import { useInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { copy, countLine, tr } from "./config"
import { DocIcon } from "./icons"
import { Tile, TileGrid } from "./tile"

interface Server {
  name: string
  on: boolean
  connected: boolean | null
  summary: string
  skipped: boolean
}

const flatten = (v: McpView): Server[] =>
  [
    ...v.installed.map((s) => ({ name: s.name, on: true, connected: s.connected, summary: s.summary, skipped: false })),
    ...v.available.map((s) => ({ name: s.name, on: false, connected: null, summary: s.description, skipped: s.skipped })),
  ].sort((a, b) => a.name.localeCompare(b.name))

/** `?dir=mcp` (and `&item=mcp:<name>`): every server `claude mcp list` knows, a switch each. */
export default function Mcp({ lang, name }: { lang: Locale; name?: string }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const inApp = useInMachine()
  const [servers, setServers] = useState<Server[] | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)

  const load = useCallback(async () => {
    try {
      setServers(flatten(await mcpList()))
    } catch (e) {
      setMessage({ ok: false, text: String(e) })
      setServers([])
    }
  }, [])

  useEffect(() => {
    if (inApp) void load()
  }, [inApp, load])

  async function flip(s: Server, next: boolean) {
    setBusy(s.name)
    setMessage(null)
    try {
      const out: WriteOutcome = await (next ? mcpAdd(s.name) : mcpRemove(s.name))
      setMessage({
        ok: out.ok,
        text: out.rolled_back ? t(copy.mcpRestored) : `${t(copy.saved)} · ${t(copy.health)} ${out.verdict}`,
      })
    } catch (e) {
      setMessage({ ok: false, text: String(e) })
    }
    await load()
    setBusy(null)
  }

  const quiet = (m: { ok: boolean; text: string }) => (
    <p className={`text-sm ${m.ok ? "text-muted-foreground" : "text-destructive"}`}>{m.text}</p>
  )

  if (inApp === null) return <PageShell title="MCP">{null}</PageShell>
  if (!inApp)
    return (
      <PageShell title="MCP">
        <p className="text-muted-foreground text-sm">{t(copy.mcpOutside)}</p>
      </PageShell>
    )

  const loading = servers === null
  const toggle = (s: Server) => ({
    on: s.on,
    busy: busy === s.name,
    label: s.name,
    onChange: (next: boolean) => void flip(s, next),
  })

  if (name) {
    const s = servers?.find((x) => x.name === name)
    return (
      <PageShell title={name}>
        {loading && <p className="text-muted-foreground text-sm">{t(copy.loading)}</p>}
        {!loading && !s && <div className="bg-background text-muted-foreground w-fit rounded-2xl p-4 text-sm">{t(copy.notFound)}</div>}
        {s && (
          <>
            <div className="flex flex-wrap items-center gap-4">
              <span className="me-3 scale-125 origin-[left_center] rtl:origin-[right_center]">
                <DocIcon glyph="mcp" dimmed={!s.on} />
              </span>
              <div className="flex min-w-0 flex-col gap-0.5">
                <h2 dir="ltr" className="text-start text-xl font-semibold break-all">
                  {s.name}
                </h2>
                <p className="text-muted-foreground text-sm">
                  {t({ en: "MCP server", ar: "خادم MCP" })}
                  {" · "}
                  {s.on
                    ? s.connected === true
                      ? t(copy.connected)
                      : s.connected === false
                        ? t(copy.notConnected)
                        : t(copy.unknown)
                    : t({ en: "Not registered", ar: "غير مسجّل" })}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-3">
              <Switch checked={s.on} busy={busy === s.name} aria-label={s.name} onCheckedChange={(n) => void flip(s, n)} />
            </div>
            {message && quiet(message)}
            {(s.summary || s.skipped) && (
              <div className="bg-background flex flex-col gap-1 rounded-2xl p-4 text-sm">
                {s.summary && <p dir="ltr" className="break-words text-start">{s.summary}</p>}
                {s.skipped && <p className="text-muted-foreground text-xs">{t(copy.skipped)}</p>}
              </div>
            )}
          </>
        )}
      </PageShell>
    )
  }

  return (
    <PageShell title="MCP">
      {loading && <p className="text-muted-foreground text-sm">{t(copy.loading)}</p>}
      {servers && servers.length === 0 && !message && <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>}
      <TileGrid>
        {servers?.map((s) => (
          <Tile
            key={s.name}
            href={`/${lang}/catalog/?dir=mcp&item=${encodeURIComponent(`mcp:${s.name}`)}`}
            icon={<DocIcon glyph="mcp" dimmed={!s.on} />}
            name={s.name}
            toggle={toggle(s)}
          />
        ))}
      </TileGrid>
      {message && quiet(message)}
      {servers && servers.length > 0 && (
        <p className="text-muted-foreground text-xs">{countLine(isAr, servers.length, servers.filter((s) => s.on).length)}</p>
      )}
    </PageShell>
  )
}
