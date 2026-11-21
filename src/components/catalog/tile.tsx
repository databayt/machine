"use client"

import Link from "next/link"

import { cn } from "@/lib/utils"
import { Switch } from "@/components/ui/switch"

import { DocIcon, FolderIcon } from "./icons"
import type { Entry } from "./types"

/** Folders on disk (a skill, a department) draw as a folder; the rest as a document. */
export function EntryIcon({ entry }: { entry: Pick<Entry, "dir" | "state"> }) {
  const p = {
    dimmed: entry.state === "available",
    off: entry.state === "disabled",
    personal: entry.state === "personal",
    update: entry.state === "outdated",
  }
  if (entry.dir === "skills" || entry.dir === "departments") return <FolderIcon {...p} />
  return <DocIcon glyph={entry.dir === "hooks" ? "sh" : "md"} {...p} />
}

export const isOn = (s: Entry["state"]) => s === "installed" || s === "outdated" || s === "personal"

/**
 * An icon with its name under it. The whole tile is the link target (`after:` stretches it); the
 * optional switch sits above that layer, so neither swallows the other.
 */
export function Tile({
  href,
  icon,
  name,
  sub,
  toggle,
  className,
}: {
  href: string
  icon: React.ReactNode
  name: string
  sub?: string
  toggle?: { on: boolean; busy: boolean; label: string; onChange: (on: boolean) => void }
  className?: string
}) {
  return (
    <li
      className={cn(
        "group relative flex w-28 flex-col items-center gap-1.5 rounded-xl p-2 transition-colors",
        "hover:bg-foreground/[0.06] focus-within:bg-foreground/[0.06]",
        className,
      )}
    >
      <Link href={href} className="flex flex-col items-center gap-1.5 outline-none after:absolute after:inset-0 after:rounded-xl">
        {icon}
        <span dir="ltr" className="line-clamp-2 w-full break-all text-center text-xs leading-tight font-medium">
          {name}
        </span>
        {sub && <span className="text-muted-foreground -mt-1 text-[11px] leading-none">{sub}</span>}
      </Link>
      {toggle && (
        <span className="relative z-10">
          <Switch checked={toggle.on} busy={toggle.busy} aria-label={toggle.label} onCheckedChange={toggle.onChange} />
        </span>
      )}
    </li>
  )
}

export function TileGrid({ children }: { children: React.ReactNode }) {
  return <ul className="flex flex-wrap content-start gap-x-2 gap-y-3">{children}</ul>
}
