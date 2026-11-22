"use client"

import Link from "next/link"

import type { Locale } from "@/components/local/config"

import { CLAUDE_DIRS } from "../shell/nav"
import { useInMachine } from "../shell/content"
import { PageShell } from "../shell/page"
import { copy, tr } from "./config"
import { DocIcon, FolderIcon } from "./icons"
import { Tile, TileGrid } from "./tile"
import type { TreeState } from "./use-tree"

const FILES = [
  { name: "CLAUDE.md", glyph: "md" },
  { name: "settings.json", glyph: "json" },
  { name: ".env", glyph: "env" },
] as const

/** `/library/`: the five folders of `~/.claude`, then its three loose files. */
export default function Root({ lang, state }: { lang: Locale; state: TreeState }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const inApp = useInMachine()
  const { entries, error } = state
  const count = (dir: string): string | undefined => {
    if (!inApp) return undefined
    if (!entries) return undefined
    return String(entries.filter((e) => e.dir === dir && e.state !== "available").length)
  }

  return (
    <PageShell title="Library">
      {error && <p className="text-destructive text-sm">{error}</p>}
      <TileGrid>
        {CLAUDE_DIRS.map((d) => (
          <Tile key={d} href={`/${lang}/library/?dir=${d}`} icon={<FolderIcon />} name={d} sub={count(d)} />
        ))}
        {inApp &&
          FILES.map((f) => <Tile key={f.name} href={`/${lang}/config/`} icon={<DocIcon glyph={f.glyph} />} name={f.name} />)}
      </TileGrid>
      {inApp === false && (
        <Link
          href={`/${lang}/`}
          className="inline-flex h-9 w-fit items-center rounded-full bg-foreground px-4 text-sm font-medium text-background transition-colors hover:bg-foreground/90"
        >
          {t(copy.getMachine)}
        </Link>
      )}
    </PageShell>
  )
}
