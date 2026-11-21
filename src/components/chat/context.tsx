"use client";

import { useCallback, useEffect, useState } from "react";
import { BookIcon, BriefcaseIcon, CheckIcon, ChevronDownIcon, SearchIcon } from "lucide-react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { copy as menuCopy } from "../menu/config";
import { loadRepos } from "../menu/data";
import { useDepartment } from "../menu/use-department";
import { getProfile, type MachineProfile } from "../profile/actions";
import { loadSettings, saveSettings } from "../settings/actions";
import { tr } from "./config";

/**
 * The working context of a run: the department (from Settings) and the repository, remembered per
 * department in `settings.repo_by_department`. The repository menu lists the department's repos that
 * are cloned here (and that this machine's profile lists, when it lists any); "" means all repos.
 * With no choice saved, hogwarts is the default whenever it is offered here.
 */
const DEFAULT_REPO = "hogwarts";

export function useWorkingContext(enabled: boolean): {
  profile: MachineProfile | null;
  dept: string;
  chooseDept: (name: string) => void;
  repos: string[];
  repo: string;
  setRepo: (repo: string) => void;
} {
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [local, setLocal] = useState<Set<string> | null>(null);
  const [saved, setSaved] = useState<Record<string, string>>({});
  const { dept, choose } = useDepartment();

  useEffect(() => {
    if (!enabled) return;
    getProfile().then(setProfile).catch(() => setProfile(null));
    loadRepos().then((rows) => setLocal(new Set(rows.filter((r) => r.local).map((r) => r.name))));
    loadSettings().then((s) => s && setSaved(s.repo_by_department)).catch(() => {});
  }, [enabled]);

  const current = profile?.departments.find((d) => d.name === dept);
  const listed = profile?.repos ?? [];
  const usable = (n: string) => !!local?.has(n) && (listed.length === 0 || listed.includes(n));
  const repos = (current?.repos ?? []).map((r) => r.name).filter(usable);
  // The run accepts a repo outside the department only when the profile lists it.
  if (listed.includes(DEFAULT_REPO) && usable(DEFAULT_REPO) && !repos.includes(DEFAULT_REPO)) repos.unshift(DEFAULT_REPO);
  const pick = saved[dept] ?? DEFAULT_REPO;
  const repo = repos.includes(pick) ? pick : "";

  const setRepo = useCallback(
    (next: string) => {
      setSaved((all) => ({ ...all, [dept]: next }));
      void (async () => {
        const now = await loadSettings();
        if (!now) return;
        const map = { ...now.repo_by_department };
        map[dept] = next; // "" is a choice too: all repos, not the hogwarts default
        await saveSettings({ ...now, repo_by_department: map }).catch(() => {});
      })();
    },
    [dept],
  );

  return { profile, dept, chooseDept: (name) => void choose(name), repos, repo, setRepo };
}

/** A trigger in the row under the composer: icon, label, chevron; tinted while its menu is open. */
const trigger =
  "inline-flex min-w-0 items-center gap-1.5 rounded-lg px-2 py-1 transition-colors hover:bg-foreground/5 hover:text-foreground disabled:opacity-50 data-[state=open]:bg-foreground/5 data-[state=open]:text-foreground";
/** A row inside a menu. */
const row =
  "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-start text-sm text-foreground transition-colors hover:bg-muted";

/** `▣ Department ▾   ▤ Repository ▾`: two selects under the composer, each opening its own menu. */
export function ContextLine({
  lang,
  profile,
  dept,
  onDept,
  repos,
  repo,
  onRepo,
  disabled,
}: {
  lang: Locale;
  profile: MachineProfile | null;
  dept: string;
  onDept: (name: string) => void;
  repos: string[];
  repo: string;
  onRepo: (repo: string) => void;
  disabled?: boolean;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const [deptOpen, setDeptOpen] = useState(false);
  const [repoOpen, setRepoOpen] = useState(false);
  const [query, setQuery] = useState("");
  const departments = profile?.departments ?? [];
  const current = departments.find((d) => d.name === dept);
  if (!current) return null;
  const title = (d: (typeof departments)[number]) => (isAr && d.title_ar) || d.title;
  const q = query.trim().toLowerCase();
  const others = repos.filter((r) => r !== repo && (!q || r.toLowerCase().includes(q)));
  const pick = (r: string) => {
    setRepoOpen(false);
    setQuery("");
    onRepo(r);
  };

  return (
    <div className="mt-2 flex min-w-0 items-center gap-2 px-1.5 text-sm text-foreground/80">
      <Popover open={deptOpen} onOpenChange={setDeptOpen}>
        <PopoverTrigger asChild>
          <button type="button" disabled={disabled} aria-label={t(menuCopy.department)} className={trigger}>
            <BriefcaseIcon className="size-4 shrink-0" aria-hidden />
            <span className="truncate">{title(current)}</span>
            <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent side="top" align="start" dir={isAr ? "rtl" : "ltr"} className="w-64 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
          <ul>
            {departments.map((d) => (
              <li key={d.name}>
                <button
                  type="button"
                  onClick={() => {
                    setDeptOpen(false);
                    if (d.name !== dept) onDept(d.name);
                  }}
                  className={row}
                >
                  <BriefcaseIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
                  <span className="flex-1 truncate">{title(d)}</span>
                  {d.name === dept && <CheckIcon className="size-4 shrink-0" aria-hidden />}
                </button>
              </li>
            ))}
          </ul>
        </PopoverContent>
      </Popover>

      <Popover
        open={repoOpen}
        onOpenChange={(o) => {
          setRepoOpen(o);
          if (!o) setQuery("");
        }}
      >
        <PopoverTrigger asChild>
          <button type="button" disabled={disabled} aria-label={t(menuCopy.repository)} className={trigger}>
            <BookIcon className="size-4 shrink-0" aria-hidden />
            <span dir="ltr" className="truncate">{repo || t(menuCopy.allRepos)}</span>
            <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent side="top" align="start" dir={isAr ? "rtl" : "ltr"} className="w-80 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
          <label className="flex items-center gap-2 rounded-lg border border-border px-2.5 py-1.5 focus-within:border-foreground/40">
            <SearchIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            <input
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && others[0]) pick(others[0]);
              }}
              placeholder={t(menuCopy.searchRepos)}
              className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
            />
          </label>
          {repo && (
            <div className="mt-1.5 flex items-center gap-2.5 rounded-lg bg-muted px-2.5 py-1.5 text-sm">
              <span dir="ltr" className="flex-1 truncate">{repo}</span>
              <CheckIcon className="size-4 shrink-0" aria-hidden />
            </div>
          )}
          {others.length > 0 && (
            <ul className="mt-1.5 max-h-64 overflow-y-auto border-t border-border/60 pt-1.5">
              {others.map((r) => (
                <li key={r}>
                  <button type="button" onClick={() => pick(r)} className={row}>
                    <span dir="ltr" className="truncate">{r}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          {repos.length === 0 && <p className="px-2.5 pb-1 pt-2 text-xs text-muted-foreground">{t(menuCopy.noRepoHere)}</p>}
          <div className="mt-1.5 border-t border-border/60 pt-1.5">
            <button type="button" onClick={() => pick("")} className={row}>
              <span className="flex-1 truncate">{t(menuCopy.allRepos)}</span>
              {repo === "" && <CheckIcon className="size-4 shrink-0" aria-hidden />}
            </button>
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
