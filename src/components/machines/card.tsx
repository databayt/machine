"use client"

import Link from "next/link"

import { cn } from "@/lib/utils"
import { Switch } from "@/components/ui/switch"

import type { Copy } from "../chat/config"
import type { MachineStatus } from "../network/types"
import { Logo } from "../plugins/card"
import { DEPARTMENTS } from "../plugins/registry"
import { copy } from "./config"
import { summary } from "./format"

type T = (c: Copy) => string

export function OnlineDot({ online, t }: { online: boolean; t: T }) {
  return (
    <span className="inline-flex items-center gap-1.5" title={t(online ? copy.online : copy.offline)}>
      <span aria-hidden className={cn("size-2 shrink-0 rounded-full", online ? "bg-emerald-500" : "bg-muted-foreground/40")} />
      <span className="sr-only">{t(online ? copy.online : copy.offline)}</span>
    </span>
  )
}

const OS: Record<string, string> = { macos: "macOS", darwin: "macOS", windows: "Windows", linux: "Linux" }

/** "macOS · Tech, Video" */
export function osAndDepartments(m: MachineStatus, t: T): string {
  const depts = m.departments.map((d) => {
    const x = DEPARTMENTS.find((y) => y.name === d)
    return x ? t(x.title) : d
  })
  return [OS[m.os.toLowerCase()] ?? m.os, depts.join(", ")].filter(Boolean).join(" · ")
}

/** One machine, in the Plugins card style: the whole card opens the detail; the switch (this machine only) sits above it. */
export function MachineCard({
  m, lang, t, self, sharing, onShare,
}: {
  m: MachineStatus
  lang: string
  t: T
  self: boolean
  sharing: boolean
  onShare?: (on: boolean) => void
}) {
  const isAr = lang === "ar"
  return (
    <div className="group bg-background hover:bg-background/60 relative flex min-w-0 items-start gap-3 rounded-2xl p-4 transition-colors">
      <Logo c={{ id: m.login, name: m.name, blurb: copy.title }} />
      <Link
        href={`/${lang}/machines/?item=${encodeURIComponent(m.login)}`}
        className="flex min-w-0 flex-1 flex-col gap-0.5 outline-none after:absolute after:inset-0 after:rounded-2xl focus-visible:after:ring-2 focus-visible:after:ring-ring"
      >
        <span className="flex items-center gap-2 truncate font-semibold">
          <bdi className="truncate">{m.name}</bdi>
          <OnlineDot online={m.online} t={t} />
          {self && <span className="bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-xs font-normal">{t(copy.thisMachine)}</span>}
        </span>
        <span className="text-muted-foreground truncate text-sm">{osAndDepartments(m, t)}</span>
        <span className="text-muted-foreground truncate text-sm">{summary(m.resources, isAr)}</span>
      </Link>
      {self && onShare && (
        <span className="relative z-10 flex shrink-0 items-center pt-1" title={t(copy.shareTitle)}>
          <Switch checked={sharing} onCheckedChange={onShare} aria-label={t(copy.shareTitle)} />
        </span>
      )}
    </div>
  )
}
