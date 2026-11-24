"use client"

import { Check, ExternalLink, Loader2 } from "lucide-react"
import { useCallback, useEffect, useState } from "react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"

import { tr, type Copy } from "../chat/config"
import { Logo } from "../plugins/card"
import { setSecret } from "../profile/actions"
import { readinessFix } from "../readiness/actions"
import { OpenInMachine, useInMachine } from "../shell/content"
import { loadSettings, saveSettings } from "../settings/actions"
import type { FallbackPolicy, ModelPolicy } from "../settings/types"
import { PageShell } from "../shell/page"
import { INSTALL_STEP, KEY_NAME, brainApply, brainMintKey, brainStatus } from "./actions"
import { copy } from "./config"
import { Providers } from "./providers"
import type { Brain, BrainView, RouteStatus } from "./types"

const CARDS: { id: Brain; name: Copy; blurb: Copy; logo?: string; links: { href: string; label: Copy }[] }[] = [
  { id: "claude", name: copy.claudeName, blurb: copy.claudeBlurb, logo: "/logos/claude.svg", links: [] },
  {
    id: "nvidia",
    name: copy.nvidiaName,
    blurb: copy.nvidiaBlurb,
    links: [{ href: "https://build.nvidia.com/settings/api-keys", label: copy.getNvidiaKey }],
  },
  {
    id: "openrouter",
    name: copy.openrouterName,
    blurb: copy.openrouterBlurb,
    links: [
      { href: "https://openrouter.ai/settings/keys", label: copy.getOpenrouterKey },
      { href: "https://openrouter.ai/settings/credits", label: copy.openrouterCredits },
    ],
  },
  {
    id: "omniroute",
    name: copy.omnirouteName,
    blurb: copy.omnirouteBlurb,
    links: [{ href: "http://localhost:20128", label: copy.omnirouteDashboard }],
  },
]

const pill = "text-sm font-medium rounded-full px-3 py-1.5 transition-colors disabled:opacity-50"

/** Settings → Model: Claude (default) and three free routes, each with its status and "Use this". */
export default function BrainContent({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const [view, setView] = useState<BrainView | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  // The Providers drill-down swaps in for the route cards; nothing stacks below them.
  const [screen, setScreen] = useState<"routes" | "providers">("routes")

  const reload = useCallback(async () => {
    const v = await brainStatus()
    if (v) setView(v)
  }, [])

  useEffect(() => {
    if (!inApp) return
    reload().catch((e) => setMessage(String(e)))
  }, [inApp, reload])

  if (inApp === null) return null
  return (
    <PageShell title={t(copy.title)}>
      {!inApp ? (
        <OpenInMachine lang={lang} />
      ) : screen === "providers" ? (
        <Providers lang={lang} t={t} onBack={() => { setScreen("routes"); void reload() }} />
      ) : (
        <>
          <p className="text-muted-foreground max-w-xl text-sm">{t(copy.intro)}</p>
          <Policies t={t} />
          {message && <p className="text-destructive text-sm">{message}</p>}
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {CARDS.map((c) => {
              const status = view?.routes.find((r) => r.route === c.id)
              return (
                <RouteCard
                  key={c.id}
                  card={c}
                  status={status}
                  current={view?.current === c.id}
                  t={t}
                  onChanged={reload}
                  onProviders={c.id === "omniroute" ? () => setScreen("providers") : undefined}
                  onApply={async () => {
                    setMessage(null)
                    try {
                      setView(await brainApply(c.id))
                    } catch (e) {
                      setMessage(String(e))
                    }
                  }}
                />
              )
            })}
          </div>
        </>
      )}
    </PageShell>
  )
}

function Status({ ok, children }: { ok: boolean; children: React.ReactNode }) {
  return (
    <li className="flex items-center gap-1.5">
      <span className={cn("size-1.5 shrink-0 rounded-full", ok ? "bg-emerald-500" : "bg-muted-foreground/40")} aria-hidden />
      {children}
    </li>
  )
}

function RouteCard({
  card,
  status,
  current,
  t,
  onApply,
  onChanged,
  onProviders,
}: {
  card: (typeof CARDS)[number]
  status: RouteStatus | undefined
  current: boolean
  t: (c: Copy) => string
  onApply: () => Promise<void>
  onChanged: () => Promise<void>
  /** Only the OmniRoute card has it: opens the Providers screen. */
  onProviders?: () => void
}) {
  const free = card.id !== "claude"
  const [key, setKey] = useState("")
  const [busy, setBusy] = useState<"key" | "install" | "use" | "mint" | null>(null)
  const [error, setError] = useState<string | null>(null)
  const name = t(card.name)

  async function run(kind: "key" | "install" | "use" | "mint", job: () => Promise<void>) {
    setBusy(kind)
    setError(null)
    try {
      await job()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  const saveKey = () =>
    run("key", async () => {
      if (card.id === "claude" || !key.trim()) return
      await setSecret(KEY_NAME[card.id], key)
      setKey("")
      await onChanged()
    })

  const install = () =>
    run("install", async () => {
      if (card.id === "claude") return
      const out = await readinessFix(INSTALL_STEP[card.id])
      if (!out.ok) throw new Error(out.message ?? "install failed")
      await onChanged()
    })

  // OmniRoute's own key is made by kun's script, not pasted: one tap runs `omni.mjs mint-key`.
  const mint = () =>
    run("mint", async () => {
      await brainMintKey()
      await onChanged()
    })

  const ready = !free || (!!status?.installed && !!status?.key_set)

  return (
    <div className={cn("bg-background flex min-w-0 flex-col gap-3 rounded-2xl p-4", current && "ring-foreground ring-2")}>
      <div className="flex items-start gap-3">
        <Logo c={{ id: card.id, name, logo: card.logo, blurb: card.blurb }} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate font-semibold">
            <bdi>{name}</bdi>
          </span>
          <span className="text-muted-foreground text-sm">{t(card.blurb)}</span>
        </div>
        {current && (
          <span className="bg-foreground text-background flex shrink-0 items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium">
            <Check className="size-3.5" aria-hidden />
            {t(copy.using)}
          </span>
        )}
      </div>

      {free && status && (
        <ul className="text-muted-foreground flex flex-col gap-1 text-sm">
          <Status ok={status.installed}>
            {status.installed ? t(copy.installed) : t(copy.notInstalled)}
            {!status.installed && (
              <button type="button" onClick={() => void install()} disabled={busy !== null} className="text-primary ms-2 font-medium disabled:opacity-50">
                {busy === "install" ? t(copy.installing) : t(copy.install)}
              </button>
            )}
          </Status>
          <Status ok={status.key_set}>
            {status.key_set ? t(copy.keySet) : t(copy.keyMissing)}
            {card.id === "omniroute" && !status.key_set && (
              <button type="button" onClick={() => void mint()} disabled={busy !== null} className="text-primary ms-2 font-medium disabled:opacity-50">
                {busy === "mint" ? t(copy.creatingKey) : t(copy.createKey)}
              </button>
            )}
          </Status>
          <Status ok={status.proxy_up}>{status.proxy_up ? t(copy.proxyUp) : t(copy.proxyDown)}</Status>
        </ul>
      )}

      {free && (
        <form
          className="flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            void saveKey()
          }}
        >
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            dir="ltr"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={status?.key_set ? t(copy.keyReplace) : t(copy.keyPlaceholder)}
            aria-label={t(copy.keyPlaceholder)}
            className="bg-muted h-10 min-w-0 flex-1 rounded-full px-4 text-sm outline-none"
          />
          <button type="submit" disabled={!key.trim() || busy !== null} className={cn(pill, "bg-muted hover:bg-muted/70")}>
            {busy === "key" ? <Loader2 className="size-4 animate-spin" aria-hidden /> : t(copy.save)}
          </button>
        </form>
      )}

      {card.links.length > 0 && (
        <div className="flex flex-col gap-1">
          {card.links.map((l) => (
            <a key={l.href} href={l.href} target="_blank" rel="noreferrer" className="text-primary inline-flex items-center gap-1.5 text-sm font-medium">
              <ExternalLink className="size-3.5 shrink-0" aria-hidden />
              {t(l.label)}
            </a>
          ))}
        </div>
      )}

      {onProviders && (
        <button type="button" onClick={onProviders} className={cn(pill, "bg-muted hover:bg-muted/70 self-start")}>
          {t(copy.providers)}
        </button>
      )}

      {free && <p className="text-muted-foreground text-xs">{t(copy.privacy)}</p>}
      {error && <p className="text-destructive text-sm">{error}</p>}

      <button
        type="button"
        disabled={current || !ready || busy !== null}
        onClick={() => void run("use", onApply)}
        className={cn(pill, "mt-auto self-start", current ? "bg-muted text-muted-foreground" : "bg-foreground text-background hover:bg-foreground/90")}
      >
        {busy === "use" ? <Loader2 className="size-4 animate-spin" aria-hidden /> : current ? t(copy.using) : t(copy.use)}
      </button>
    </div>
  )
}

type PolicyKey = "model_policy" | "fallback"

const MODEL_CHOICES: { value: ModelPolicy; name: Copy; note: Copy }[] = [
  { value: "auto", name: copy.policyAuto, note: copy.policyAutoNote },
  { value: "opus", name: copy.policyOpus, note: copy.policyOpusNote },
  { value: "sonnet", name: copy.policySonnet, note: copy.policySonnetNote },
]
const FALLBACK_CHOICES: { value: FallbackPolicy; name: Copy; note: Copy }[] = [
  { value: "auto", name: copy.fallbackAuto, note: copy.fallbackAutoNote },
  { value: "ask", name: copy.fallbackAsk, note: copy.fallbackAskNote },
  { value: "off", name: copy.fallbackOff, note: copy.fallbackOffNote },
]

/** Model policy and Fallback: two small radio groups, saved to Settings the moment one is picked. */
function Policies({ t }: { t: (c: Copy) => string }) {
  const [saved, setSaved] = useState<{ model_policy: ModelPolicy; fallback: FallbackPolicy }>({ model_policy: "auto", fallback: "auto" })
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadSettings()
      .then((s) => s && setSaved({ model_policy: s.model_policy, fallback: s.fallback }))
      .catch((e) => setError(String(e)))
  }, [])

  async function pick<K extends PolicyKey>(key: K, value: (typeof saved)[K]) {
    setError(null)
    const before = saved
    setSaved({ ...saved, [key]: value })
    try {
      // Read the stored settings fresh so nothing else on the page is overwritten.
      const fresh = await loadSettings()
      if (!fresh) return
      const out = await saveSettings({ ...fresh, [key]: value })
      setSaved({ model_policy: out.model_policy, fallback: out.fallback })
    } catch (e) {
      setSaved(before)
      setError(String(e))
    }
  }

  return (
    <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
      <Group title={t(copy.policyTitle)} intro={t(copy.policyIntro)} name="model_policy" value={saved.model_policy} choices={MODEL_CHOICES} t={t} onPick={(v) => void pick("model_policy", v)} />
      <Group title={t(copy.fallbackTitle)} intro={t(copy.fallbackIntro)} name="fallback" value={saved.fallback} choices={FALLBACK_CHOICES} t={t} onPick={(v) => void pick("fallback", v)} />
      {error && <p className="text-destructive text-sm md:col-span-2">{error}</p>}
    </div>
  )
}

function Group<V extends string>({
  title,
  intro,
  name,
  value,
  choices,
  t,
  onPick,
}: {
  title: string
  intro: string
  name: string
  value: V
  choices: { value: V; name: Copy; note: Copy }[]
  t: (c: Copy) => string
  onPick: (v: V) => void
}) {
  return (
    <fieldset className="bg-background flex min-w-0 flex-col gap-2 rounded-2xl p-4">
      <legend className="sr-only">{title}</legend>
      <span className="font-semibold">{title}</span>
      <span className="text-muted-foreground text-sm">{intro}</span>
      {choices.map((c) => (
        <label key={c.value} className={cn("flex cursor-pointer items-start gap-3 rounded-xl p-2 transition-colors", value === c.value ? "bg-muted" : "hover:bg-muted/50")}>
          <input type="radio" name={name} checked={value === c.value} onChange={() => onPick(c.value)} className="mt-1 size-4 shrink-0 accent-foreground" />
          <span className="flex min-w-0 flex-col gap-0.5">
            <span className="text-sm font-medium">{t(c.name)}</span>
            <span className="text-muted-foreground text-xs">{t(c.note)}</span>
          </span>
        </label>
      ))}
    </fieldset>
  )
}
