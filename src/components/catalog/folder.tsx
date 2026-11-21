"use client"

import type { Locale } from "@/components/local/config"

import { PageShell } from "../shell/page"
import { copy, countLine, dirLabel, tr } from "./config"
import { disable, enable } from "./actions"
import { EntryIcon, isOn, Tile, TileGrid } from "./tile"
import type { TreeState } from "./use-tree"
import { itemHref } from "./href"

/** `?dir=skills|agents|rules|hooks|departments`: one tile per entry, the state in the icon. */
export default function Folder({ lang, dir, state }: { lang: Locale; dir: string; state: TreeState }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const { entries, error, busy, actionError, run } = state
  const shown = (entries ?? []).filter((e) => e.dir === dir).sort((a, b) => a.name.localeCompare(b.name))
  const on = shown.filter((e) => isOn(e.state)).length

  return (
    <PageShell title={t(dirLabel[dir] ?? dirLabel.skills)}>
      {!entries && !error && <p className="text-muted-foreground text-sm">{t(copy.loading)}</p>}
      {entries && shown.length === 0 && <p className="text-muted-foreground text-sm">{t(copy.empty)}</p>}
      <TileGrid>
        {shown.map((e) => (
          <Tile
            key={e.id}
            href={itemHref(lang, e)}
            icon={<EntryIcon entry={e} />}
            name={e.name}
            toggle={
              e.can_toggle
                ? {
                    on: e.state !== "disabled",
                    busy: busy === e.id,
                    label: e.name,
                    onChange: (next) => void run(e.id, () => (next ? enable(e.id) : disable(e.id))),
                  }
                : undefined
            }
          />
        ))}
      </TileGrid>
      {(error || actionError) && <p className="text-destructive text-sm">{error ?? actionError}</p>}
      {entries && shown.length > 0 && <p className="text-muted-foreground text-xs">{countLine(isAr, shown.length, on)}</p>}
    </PageShell>
  )
}
