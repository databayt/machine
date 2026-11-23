"use client"

import { Loader2, MoreHorizontal, Plus } from "lucide-react"
import Link from "next/link"
import { useState } from "react"

import { cn } from "@/lib/utils"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"

import type { Copy } from "../chat/config"
import { promptHref } from "../menu/config"
import { copy } from "./config"
import { KINDS, type Connector } from "./registry"

type T = (c: Copy) => string

/** What a card can do: add it, or (once installed) drop it. Supplied by the page, which owns `claude mcp`. */
export interface Actions {
  installed: (c: Connector) => boolean
  /** Adds it in place when kun's catalog has it; otherwise the action opens its page to add it there. */
  canAdd: (c: Connector) => boolean
  /** Drops it in place when it is a local server; connectors and plugins are managed on their page. */
  canRemove: (c: Connector) => boolean
  add: (c: Connector) => Promise<void>
  remove: (c: Connector) => Promise<void>
}

/** What a card shows: a plugin or an app. */
export interface Item {
  id: string
  name: string
  /** `/logos/<file>`; absent: a lettered tile. */
  logo?: string
  blurb: Copy
}

/** The original mark, no frame. A white backing only in dark mode, where black marks would vanish. */
export function Logo({ c, size = 40 }: { c: Item; size?: number }) {
  const inner = Math.round(size * 0.7)
  return (
    <span
      className="flex shrink-0 items-center justify-center rounded-xl dark:bg-white"
      style={{ width: size, height: size }}
    >
      {c.logo ? (
        // eslint-disable-next-line @next/next/no-img-element -- static export, no image optimizer
        <img src={c.logo} alt="" className="object-contain" style={{ width: inner, height: inner }} />
      ) : (
        <span dir="ltr" className="font-semibold text-neutral-800 dark:text-neutral-800" style={{ fontSize: size * 0.5 }}>
          {c.name.charAt(0).toUpperCase()}
        </span>
      )}
    </span>
  )
}

/** `Paid, optional · $6 a month`: the one muted line a paid card shows. Free tools stay the default. */
export const stagedLine = (c: Connector, t: T) => (c.staged ? `${t(copy.needsDecision)} · ${t(c.staged.price)}` : "")

/**
 * The quick fix for a paid tool, as an English prompt the chat carries to the agent: subscribe in the person's own
 * name (the `behalf` skill), then store the key and register it with kun's script. Paying is each person's choice.
 */
export const setupPrompt = (c: Connector): string =>
  `Set up ${c.name} for me, paid on my own account. Open its pricing page, subscribe on my behalf with the behalf skill (ask me before any payment), then run \`node ~/kun/.claude/scripts/video-tools.mjs setup ${c.setup ?? c.id}\` and give it the API key it needs (never print the key). Finish with \`video-tools.mjs status ${c.setup ?? c.id}\`.`

export const kindLabel = (c: Connector, t: T) => t(KINDS.find((k) => k.id === c.kind)!.label)

export const iconButton =
  "text-muted-foreground hover:text-foreground hover:bg-muted flex size-8 shrink-0 items-center justify-center rounded-full transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring"

/** `+` for one not installed yet; `…` with Remove / Open for one already installed. */
export function Action({ c, actions, t, className }: { c: Connector; actions: Actions; t: T; className?: string }) {
  const [busy, setBusy] = useState(false)
  const [open, setOpen] = useState(false)
  const run = async (f: () => Promise<void>) => {
    setBusy(true)
    try {
      await f()
    } finally {
      setBusy(false)
      setOpen(false)
    }
  }

  if (busy)
    return (
      <span className={cn(iconButton, className, "opacity-100")} aria-busy>
        <Loader2 className="size-4 animate-spin" aria-hidden />
      </span>
    )

  if (!actions.installed(c)) {
    const label = `${t(copy.add)} ${c.name}`
    return actions.canAdd(c) ? (
      <button type="button" aria-label={label} title={t(copy.add)} onClick={() => void run(() => actions.add(c))} className={cn(iconButton, className)}>
        <Plus className="size-4" aria-hidden />
      </button>
    ) : (
      <a href={c.url} target="_blank" rel="noreferrer" aria-label={label} title={t(copy.add)} className={cn(iconButton, className)}>
        <Plus className="size-4" aria-hidden />
      </a>
    )
  }

  const item = "hover:bg-muted flex w-full items-center rounded-md px-2 py-1.5 text-start text-sm transition-colors"
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" aria-label={`${t(copy.more)} ${c.name}`} className={cn(iconButton, className, open && "opacity-100")}>
          <MoreHorizontal className="size-4" aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-40 rounded-xl p-1">
        {actions.canRemove(c) && (
          <button type="button" onClick={() => void run(() => actions.remove(c))} className={cn(item, "text-destructive")}>
            {t(copy.remove)}
          </button>
        )}
        <a href={c.url} target="_blank" rel="noreferrer" onClick={() => setOpen(false)} className={item}>
          {actions.canRemove(c) ? t(copy.open) : t(copy.manage)}
        </a>
      </PopoverContent>
    </Popover>
  )
}

/** The whole card links to the detail; the action sits above that layer. Logo, text and action are top-aligned. */
export function Card({ c, href, action, note, t }: { c: Item; href: string; action?: React.ReactNode; note?: string; t: T }) {
  return (
    <div className="group bg-background hover:bg-background/60 relative flex min-w-0 items-start gap-3 rounded-2xl p-4 transition-colors">
      <Logo c={c} />
      <Link
        href={href}
        className="flex min-w-0 flex-1 flex-col gap-0.5 outline-none after:absolute after:inset-0 after:rounded-2xl focus-visible:after:ring-2 focus-visible:after:ring-ring"
      >
        <span className="truncate font-semibold">
          <bdi>{c.name}</bdi>
        </span>
        <span className="text-muted-foreground truncate text-sm">{t(c.blurb)}</span>
        {note && <span className="text-muted-foreground truncate text-xs">{note}</span>}
      </Link>
      {action}
    </div>
  )
}

/** The action's hover-reveal classes, above the card's link layer. */
export const hoverAction = "relative z-10 opacity-0 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"

export function Grid({ children }: { children: React.ReactNode }) {
  return <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">{children}</div>
}

/** The plugins' card: its action is `+` / `…`. */
export function PluginCard({ c, lang, actions, t }: { c: Connector; lang: string; actions: Actions; t: T }) {
  const href = `/${lang}/plugins/?item=${encodeURIComponent(c.id)}`
  // Staged: listed, not installed. The price stands where the installed state would be; no add, no remove.
  if (c.staged) return <Card c={c} t={t} href={href} note={stagedLine(c, t)} action={<SetUp c={c} lang={lang} t={t} className={hoverAction} />} />
  return <Card c={c} t={t} href={href} action={<Action c={c} actions={actions} t={t} className={hoverAction} />} />
}

/** A paid card's one action: Set up opens the chat with the quick fix in the box (nothing runs until Send). */
export function SetUp({ c, lang, t, className }: { c: Connector; lang: string; t: T; className?: string }) {
  return (
    <a
      href={promptHref(lang, setupPrompt(c))}
      className={cn("bg-foreground text-background hover:bg-foreground/85 rounded-full px-3 py-1 text-xs font-medium transition-colors", className)}
    >
      {t(copy.setUp)}
    </a>
  )
}
