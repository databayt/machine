"use client"

import { useState } from "react"

import type { Locale } from "@/components/local/config"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"

import { disable, enable, install, uninstall } from "./actions"
import { copy, kindLine, stateLabel, tr } from "./config"
import { dirOf, itemHref } from "./href"
import { PageShell } from "../shell/page"
import { itemName } from "../shell/nav"
import { EntryIcon, Tile, TileGrid } from "./tile"
import type { TreeState } from "./use-tree"
import type { Entry } from "./types"

const panel = "bg-background flex min-w-0 flex-col gap-2 rounded-2xl p-4"
const pill = "rounded-full"

/** `&item=<id>`: a Get Info screen. A pack opens as a folder of what it needs. */
export default function Item({ lang, id, state }: { lang: Locale; id: string; state: TreeState }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const { entries, error, busy, actionError, run } = state
  const entry = entries?.find((e) => e.id === id)

  if (!entry) {
    return (
      <PageShell title={itemName(id)}>
        {error && <p className="text-destructive text-sm">{error}</p>}
        {!entries && !error && <p className="text-muted-foreground text-sm">{t(copy.loading)}</p>}
        {entries && <div className={`${panel} text-muted-foreground w-fit text-sm`}>{t(copy.notFound)}</div>}
      </PageShell>
    )
  }

  const pack = entry.dir === "departments"
  const personal = entry.state === "personal"
  const description = (isAr && entry.description_ar) || entry.description
  const deps = entry.dependencies.map((d) => ({ id: d, entry: entries?.find((e) => e.id === d) }))

  return (
    <PageShell title={entry.name}>
      <div className="flex flex-wrap items-center gap-4">
        <span className="scale-125 origin-[left_center] rtl:origin-[right_center] me-3">
          <EntryIcon entry={entry} />
        </span>
        <div className="flex min-w-0 flex-col gap-0.5">
          <h2 dir="ltr" className="text-start text-xl font-semibold break-all">
            {entry.name}
          </h2>
          <p className="text-muted-foreground text-sm">{kindLine(isAr, entry.dir, entry.owner)}</p>
          <p className="text-muted-foreground text-sm">{t(stateLabel[entry.state])}</p>
        </div>
      </div>

      <Controls entry={entry} isAr={isAr} pack={pack} personal={personal} working={busy === entry.id} run={run} />
      {actionError && <p className="text-destructive text-sm">{actionError}</p>}

      {personal && <div className={`${panel} text-sm`}>{t(copy.personalNote)}</div>}

      {description && (
        <div className={`${panel} text-sm`}>
          <p>{description}</p>
        </div>
      )}

      {pack && (
        <TileGrid>
          {deps.map((d) => (
            <Tile
              key={d.id}
              href={itemHref(lang, d)}
              icon={<EntryIcon entry={d.entry ?? { dir: (dirOf(d.id) || "skills") as Entry["dir"], state: "available" }} />}
              name={d.entry?.name ?? itemName(d.id)}
            />
          ))}
        </TileGrid>
      )}

      {entry.files.length > 0 && (
        <div className={panel}>
          <h3 className="text-muted-foreground text-xs">{t(copy.files)}</h3>
          <ul dir="ltr" className="text-muted-foreground max-h-48 space-y-1 overflow-y-auto font-mono text-xs">
            {entry.files.map((f) => (
              <li key={f}>~/.claude/{f}</li>
            ))}
          </ul>
        </div>
      )}

      {!pack && deps.length > 0 && (
        <div className={panel}>
          <h3 className="text-muted-foreground text-xs">{t(copy.needs)}</h3>
          <TileGrid>
            {deps.map((d) => (
              <Tile
                key={d.id}
                href={itemHref(lang, d)}
                icon={<EntryIcon entry={d.entry ?? { dir: (dirOf(d.id) || "skills") as Entry["dir"], state: "available" }} />}
                name={d.entry?.name ?? itemName(d.id)}
              />
            ))}
          </TileGrid>
        </div>
      )}

      {entry.keywords.length > 0 && (
        <div className={panel}>
          <h3 className="text-muted-foreground text-xs">{t(copy.keywords)}</h3>
          <ul className="flex flex-wrap gap-1.5">
            {entry.keywords.map((k) => (
              <li key={k} dir="ltr" className="bg-muted rounded-full px-2.5 py-0.5 text-xs">
                {k}
              </li>
            ))}
          </ul>
        </div>
      )}

      {entry.version && (
        <div className={panel}>
          <h3 className="text-muted-foreground text-xs">{t(copy.version)}</h3>
          <p dir="ltr" className="font-mono text-xs">
            {entry.version.slice(0, 12)}
          </p>
        </div>
      )}
    </PageShell>
  )
}

/** The switch, Install / Update and Uninstall on one row; Uninstall asks inline first. */
function Controls({
  entry,
  isAr,
  pack,
  personal,
  working,
  run,
}: {
  entry: Entry
  isAr: boolean
  pack: boolean
  personal: boolean
  working: boolean
  run: TreeState["run"]
}) {
  const t = tr(isAr)
  const [asking, setAsking] = useState(false)
  const canInstall = entry.can_install && !personal
  const canUninstall = entry.can_uninstall && !personal
  if (!entry.can_toggle && !canInstall && !canUninstall) return null

  return (
    <div className="flex flex-wrap items-center gap-3">
      {asking ? (
        <>
          <span className="text-sm">{t(copy.confirm)}</span>
          <Button
            variant="destructive"
            size="sm"
            className={pill}
            disabled={working}
            onClick={() => {
              setAsking(false)
              void run(entry.id, () => uninstall(entry.id))
            }}
          >
            {t(copy.remove)}
          </Button>
          <Button variant="outline" size="sm" className={pill} onClick={() => setAsking(false)}>
            {t(copy.cancel)}
          </Button>
        </>
      ) : (
        <>
          {entry.can_toggle && (
            <Switch
              checked={entry.state !== "disabled"}
              busy={working}
              aria-label={entry.name}
              onCheckedChange={(next) => void run(entry.id, () => (next ? enable(entry.id) : disable(entry.id)))}
            />
          )}
          {canInstall && (
            <Button size="sm" className={pill} disabled={working} onClick={() => void run(entry.id, () => install(entry.id))}>
              {working
                ? t(copy.working)
                : entry.state === "outdated"
                  ? t(copy.update)
                  : pack
                    ? t(copy.installPack)
                    : t(copy.install)}
            </Button>
          )}
          {canUninstall && (
            <Button variant="outline" size="sm" className={pill} disabled={working} onClick={() => setAsking(true)}>
              {t(copy.uninstall)}
            </Button>
          )}
        </>
      )}
    </div>
  )
}
