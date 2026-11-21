"use client";

import { useEffect, useRef, useState } from "react";

import {
  ActivityIcon,
  BoxIcon,
  CameraIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ClapperboardIcon,
  FileTextIcon,
  FilmIcon,
  FolderOpenIcon,
  GlobeIcon,
  LanguagesIcon,
  LightbulbIcon,
  MicIcon,
  PlayIcon,
  ScissorsIcon,
  UsersIcon,
  WrenchIcon,
  type LucideIcon,
} from "lucide-react";

import { starterSource } from "../pack/actions";
import type { SourceItem, Starter } from "../pack/types";
import { copy, tr } from "./config";

const ICONS: Record<string, LucideIcon> = {
  Clapperboard: ClapperboardIcon,
  Film: FilmIcon,
  Users: UsersIcon,
  Globe: GlobeIcon,
  Languages: LanguagesIcon,
  Play: PlayIcon,
  Wrench: WrenchIcon,
  Box: BoxIcon,
  Scissors: ScissorsIcon,
  Mic: MicIcon,
  Camera: CameraIcon,
  FileText: FileTextIcon,
  Activity: ActivityIcon,
  FolderOpen: FolderOpenIcon,
};

const MAX_ROWS = 5;

/** Fills `{{token}}`s; a token with no value stays as written. */
export function fillPrompt(prompt: string, facets: Record<string, string>): string {
  return prompt.replace(/\{\{(\w+)\}\}/g, (m, k: string) => facets[k] ?? m);
}

const labelOf = (n: Starter, isAr: boolean) => (isAr ? n.label_ar : n.label_en) || n.label_en || n.label_ar;

/** A generated item as a node: its own label + facets, the source's `then` as its children. */
function fromItem(parent: Starter, item: SourceItem, i: number): Starter {
  return {
    id: `${parent.id}:${i}`,
    label_ar: item.label_ar,
    label_en: item.label_en,
    description_ar: item.description_ar,
    description_en: item.description_en,
    icon: parent.icon,
    repos: [],
    set: item.set,
    children: parent.then,
    source: null,
    then: [],
    prompt: null,
  };
}

/** Facets of a path: every `set` key, plus `<key>_label` = the chosen node's label. */
function facetsOf(path: Starter[], repo: string, isAr: boolean): Record<string, string> {
  const out: Record<string, string> = { repo };
  for (const n of path)
    for (const [k, v] of Object.entries(n.set)) {
      out[k] = v;
      out[`${k}_label`] = labelOf(n, isAr);
    }
  return out;
}

/**
 * The home starters as a drill-down: a few plain rows; tapping a branch swaps them for its
 * children, a quiet trail of small chips above shows the choices (a chip steps back to the level
 * where that choice was made, the arrow steps up one), and a leaf sends its prompt with the
 * facets filled in. Dynamic levels (`source`) load their rows on entry.
 */
export function Navigator({
  starters,
  dept,
  repo,
  isAr,
  onPick,
  disabled,
}: {
  starters: Starter[];
  dept: string;
  repo: string;
  isAr: boolean;
  onPick: (prompt: string) => void;
  disabled?: boolean;
}) {
  const t = tr(isAr);
  const [path, setPath] = useState<Starter[]>([]);
  const [more, setMore] = useState(false);
  const dir = useRef(1);
  const [loaded, setLoaded] = useState<{ key: string; items: SourceItem[] } | null>(null);

  // A new department or repo starts from the top.
  useEffect(() => {
    setPath([]);
    setMore(false);
  }, [dept, repo]);

  const current = path.at(-1);
  const source = current?.source ?? null;
  const facets = facetsOf(path, repo, isAr);
  const key = source ? `${dept}|${source}|${repo}|${facets.project ?? ""}` : "";
  useEffect(() => {
    if (!source) return;
    let alive = true;
    starterSource(dept, source, { repo, project: facets.project ?? null })
      .catch(() => [] as SourceItem[])
      .then((items) => alive && setLoaded({ key, items }));
    return () => {
      alive = false;
    };
    // `key` carries every input of the call.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const loading = !!source && loaded?.key !== key;
  const rows: Starter[] = !current
    ? starters
    : source
      ? loading
        ? []
        : (loaded?.items ?? []).map((it, i) => fromItem(current, it, i))
      : current.children;
  const shown = more ? rows : rows.slice(0, MAX_ROWS);

  const go = (next: Starter[], d: number) => {
    dir.current = d;
    setPath(next);
    setMore(false);
  };

  const choose = (n: Starter) => {
    const leaf = n.children.length === 0 && !n.source;
    if (leaf && n.prompt) {
      onPick(fillPrompt(n.prompt, facetsOf([...path, n], repo, isAr)));
      go([], -1);
      return;
    }
    go([...path, n], 1);
  };

  const Back = isAr ? ChevronRightIcon : ChevronLeftIcon;
  const sign = (dir.current > 0 ? 1 : -1) * (isAr ? -1 : 1);

  return (
    <div className="w-full">
      {path.length > 0 && (
        <div className="flex items-center gap-1 px-1 pb-1" role="navigation" aria-label={t(copy.navTrail)}>
          <button
            type="button"
            onClick={() => go(path.slice(0, -1), -1)}
            aria-label={t(copy.navBack)}
            className="flex size-7 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:text-foreground"
          >
            <Back className="size-4" aria-hidden />
          </button>
          <ol className="flex min-w-0 flex-wrap items-center gap-x-1 gap-y-0.5 text-xs text-muted-foreground">
            {path.map((n, i) => (
              <li key={n.id} className="flex items-center gap-1">
                {i > 0 && <span aria-hidden>·</span>}
                <button
                  type="button"
                  onClick={() => go(path.slice(0, i), -1)}
                  className="rounded-full px-1.5 py-0.5 transition-colors hover:text-foreground"
                >
                  <span dir="auto">{labelOf(n, isAr)}</span>
                </button>
              </li>
            ))}
          </ol>
        </div>
      )}
      <div
        key={path.map((n) => n.id).join("/")}
        className="machine-nav-in"
        style={{ "--nav-x": `${sign * 16}px` } as React.CSSProperties}
      >
        {loading ? (
          <ul className="flex flex-col" aria-busy>
            {[0, 1, 2].map((i) => (
              <li key={i} className="flex items-center gap-4 px-2 py-3.5">
                <span className="size-5 shrink-0 animate-pulse rounded-full bg-muted" />
                <span className="h-4 w-2/3 animate-pulse rounded bg-muted" />
              </li>
            ))}
          </ul>
        ) : rows.length === 0 ? (
          <p className="px-2 py-3.5 text-[15px] text-muted-foreground">{t(copy.navNothing)}</p>
        ) : (
          <ul className="flex flex-col">
            {shown.map((n) => {
              const Icon = ICONS[n.icon ?? ""] ?? LightbulbIcon;
              const branch = n.children.length > 0 || !!n.source;
              const about = (isAr ? n.description_ar : n.description_en) || "";
              return (
                <li key={n.id}>
                  <button
                    type="button"
                    onClick={() => choose(n)}
                    disabled={disabled}
                    title={about || undefined}
                    className="flex w-full min-w-0 items-center gap-4 px-2 py-3.5 text-start text-[17px] text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
                  >
                    <Icon className="size-5 shrink-0" strokeWidth={1.5} aria-hidden />
                    <span dir="auto" className="min-w-0 flex-1 truncate">
                      {labelOf(n, isAr)}
                    </span>
                    {branch && (
                      <Back className="size-4 shrink-0 opacity-50 rotate-180" aria-hidden />
                    )}
                  </button>
                </li>
              );
            })}
            {!more && rows.length > MAX_ROWS && (
              <li>
                <button
                  type="button"
                  onClick={() => setMore(true)}
                  className="px-2 py-2 text-sm text-muted-foreground transition-colors hover:text-foreground"
                >
                  {t(copy.navMore)}
                </button>
              </li>
            )}
          </ul>
        )}
      </div>
    </div>
  );
}
