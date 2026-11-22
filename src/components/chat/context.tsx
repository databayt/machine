"use client";

import { useCallback, useEffect, useState } from "react";
import { CheckIcon, ChevronDownIcon, CloudIcon, LoaderIcon, SearchIcon } from "lucide-react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { copy as menuCopy } from "../menu/config";
import { loadRepos } from "../menu/data";
import { cloneRepo } from "../repos/actions";
import { useDepartment } from "../menu/use-department";
import { getProfile, type MachineProfile } from "../profile/actions";
import { loadSettings, saveSettings } from "../settings/actions";
import { HealthLight, type DeptHealth } from "../pack/health";
import { repoMenu } from "./actions";
import { tr } from "./config";
import { DepartmentSelect, row, trigger } from "./department-select";
import { filterRepos, pickRepo, repoLabel } from "./repos";
import type { RepoEntry } from "./types";

/**
 * The working context of a run: the department (from Settings) and the repository, remembered per
 * department in `settings.repo_by_department`. The repository menu is the department's pack repos plus
 * the profile's, active in kun's registry, hogwarts first and the default (`chat/repo_menu.rs`); a repo
 * with a `word` shows it. One that is not cloned here is cloned from github.com/databayt when picked.
 */
export function useWorkingContext(enabled: boolean): {
  profile: MachineProfile | null;
  dept: string;
  chooseDept: (name: string) => Promise<boolean>;
  repos: readonly RepoEntry[];
  local: Set<string> | null;
  repo: string;
  setRepo: (repo: string) => Promise<void>;
} {
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [local, setLocal] = useState<Set<string> | null>(null);
  const [saved, setSaved] = useState<Record<string, string>>({});
  const { dept, choose } = useDepartment();
  const [repos, setRepos] = useState<readonly RepoEntry[]>([]);

  const readLocal = useCallback(
    () => loadRepos().then((rows) => setLocal(new Set(rows.filter((r) => r.local).map((r) => r.name)))),
    [],
  );

  useEffect(() => {
    if (!enabled) return;
    getProfile().then(setProfile).catch(() => setProfile(null));
    void readLocal();
    loadSettings().then((s) => s && setSaved(s.repo_by_department)).catch(() => {});
  }, [enabled, readLocal]);

  // The menu follows the department (and the profile, which loads with the page).
  useEffect(() => {
    if (!enabled || !dept) return;
    let live = true;
    void repoMenu(dept)
      .then((m) => live && setRepos(m))
      .catch(() => live && setRepos([]));
    return () => {
      live = false;
    };
  }, [enabled, dept, profile]);

  const repo = pickRepo(saved[dept], repos);

  const setRepo = useCallback(
    async (next: string) => {
      if (local && !local.has(next)) {
        await cloneRepo(next);
        await readLocal();
      }
      setSaved((all) => ({ ...all, [dept]: next }));
      const now = await loadSettings();
      if (!now) return;
      await saveSettings({ ...now, repo_by_department: { ...now.repo_by_department, [dept]: next } }).catch(() => {});
    },
    [dept, local, readLocal],
  );

  return { profile, dept, chooseDept: choose, repos, local, repo, setRepo };
}

/** GitHub's repo mark (octicon `repo`), as the hogwarts profile uses it. */
function RepoIcon({ className }: { className?: string }) {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" fill="currentColor" className={className}>
      <path d="M2 2.5A2.5 2.5 0 0 1 4.5 0h8.75a.75.75 0 0 1 .75.75v12.5a.75.75 0 0 1-.75.75h-2.5a.75.75 0 0 1 0-1.5h1.75v-2h-8a1 1 0 0 0-.714 1.7.75.75 0 1 1-1.072 1.05A2.495 2.495 0 0 1 2 11.5Zm10.5-1h-8a1 1 0 0 0-1 1v6.708A2.486 2.486 0 0 1 4.5 9h8ZM5 12.25a.25.25 0 0 1 .25-.25h3.5a.25.25 0 0 1 .25.25v3.25a.25.25 0 0 1-.4.2l-1.45-1.087a.249.249 0 0 0-.3 0L5.4 15.7a.25.25 0 0 1-.4-.2Z" />
    </svg>
  );
}

/** `▣ Department ▾   ▤ Repository ▾`: two selects under the composer, each opening its own menu. */
export function ContextLine({
  lang,
  profile,
  dept,
  onDept,
  repos,
  local,
  repo,
  onRepo,
  health,
  onAsk,
  disabled,
}: {
  lang: Locale;
  profile: MachineProfile | null;
  dept: string;
  onDept: (name: string) => void;
  repos: readonly RepoEntry[];
  /** The repos cloned here; null while unknown. */
  local: Set<string> | null;
  repo: string;
  onRepo: (repo: string) => Promise<void>;
  /** The department's health light; hidden when its pack declares none. */
  health?: DeptHealth;
  /** Sends a prompt to the agent (the home suggestions' path). */
  onAsk?: (prompt: string) => void;
  disabled?: boolean;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const [repoOpen, setRepoOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [cloning, setCloning] = useState<string | null>(null);
  const departments = profile?.departments ?? [];
  const current = departments.find((d) => d.name === dept);
  if (!current) return null;
  const others = filterRepos(repos, repo, query);
  const shownRepo = repos.find((e) => e.name === repo);
  const pick = (r: string) => {
    setRepoOpen(false);
    setQuery("");
    setCloning(local && !local.has(r) ? r : null);
    onRepo(r)
      .catch(() => {})
      .finally(() => setCloning(null));
  };
  const remote = (r: string) => local !== null && !local.has(r);

  return (
    <div className="mt-2 flex min-w-0 items-center gap-2 px-1.5 text-sm text-foreground/80">
      <DepartmentSelect
        isAr={isAr}
        options={departments.map((d) => ({ name: d.name, title: { en: d.title, ar: d.title_ar || d.title } }))}
        value={dept}
        onChange={onDept}
        disabled={disabled}
      />

      {repos.length > 0 && (
      <Popover
        open={repoOpen}
        onOpenChange={(o) => {
          setRepoOpen(o);
          if (!o) setQuery("");
        }}
      >
        <PopoverTrigger asChild>
          <button type="button" disabled={disabled || cloning !== null} aria-label={t(menuCopy.repository)} className={trigger}>
            {cloning ? (
              <LoaderIcon className="size-4 shrink-0 animate-spin" aria-hidden />
            ) : (
              <RepoIcon className="size-4 shrink-0" />
            )}
            <span dir="ltr" className="truncate">{cloning ?? (shownRepo ? repoLabel(shownRepo) : repo)}</span>
            <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent side="bottom" align="start" dir={isAr ? "rtl" : "ltr"} className="w-80 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
          <label className="flex items-center gap-2 rounded-lg border border-border px-2.5 py-1.5 focus-within:border-foreground/40">
            <SearchIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            <input
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && others[0]) pick(others[0].name);
              }}
              placeholder={t(menuCopy.searchRepos)}
              className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
            />
          </label>
          <div className="mt-1.5 flex items-center gap-2.5 rounded-lg bg-muted px-2.5 py-1.5 text-sm">
            <RepoIcon className="size-4 shrink-0 text-muted-foreground" />
            <span dir="ltr" className="flex-1 truncate">{shownRepo ? repoLabel(shownRepo) : repo}</span>
            <CheckIcon className="size-4 shrink-0" aria-hidden />
          </div>
          {others.length > 0 && (
            <ul className="mt-1.5 max-h-64 overflow-y-auto border-t border-border/60 pt-1.5">
              {others.map((e) => (
                <li key={e.name}>
                  <button type="button" onClick={() => pick(e.name)} className={row}>
                    <RepoIcon className="size-4 shrink-0 text-muted-foreground" />
                    <span dir="ltr" className="flex-1 truncate">{repoLabel(e)}</span>
                    {remote(e.name) && (
                      <CloudIcon className="size-4 shrink-0 text-muted-foreground" aria-label={t(menuCopy.notCloned)} />
                    )}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </PopoverContent>
      </Popover>
      )}

      {health && onAsk && <HealthLight lang={lang} dept={dept} state={health} onAsk={onAsk} />}
    </div>
  );
}
