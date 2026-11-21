"use client";

import { useCallback, useEffect, useState } from "react";
import {
  BriefcaseIcon,
  CheckIcon,
  ChevronDownIcon,
  ClapperboardIcon,
  CloudIcon,
  CodeXmlIcon,
  FlaskConicalIcon,
  ImageIcon,
  HandCoinsIcon,
  LifeBuoyIcon,
  LoaderIcon,
  MegaphoneIcon,
  SearchIcon,
  type LucideIcon,
} from "lucide-react";

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
import { tr } from "./config";

/**
 * The working context of a run: the department (from Settings) and the repository, remembered per
 * department in `settings.repo_by_department`. The repository menu offers the same databayt repos on
 * every machine (`CHAT_REPOS`, mirrored in `chat/rust.rs`), hogwarts first and the default. One that
 * is not cloned here is cloned from github.com/databayt when picked.
 */
export const CHAT_REPOS = ["hogwarts", "mkan", "kun", "machine", "marketing", "android-app", "ios-app"] as const;
const DEFAULT_REPO = CHAT_REPOS[0];

export function useWorkingContext(enabled: boolean): {
  profile: MachineProfile | null;
  dept: string;
  chooseDept: (name: string) => void;
  repos: readonly string[];
  local: Set<string> | null;
  repo: string;
  setRepo: (repo: string) => Promise<void>;
} {
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [local, setLocal] = useState<Set<string> | null>(null);
  const [saved, setSaved] = useState<Record<string, string>>({});
  const { dept, choose } = useDepartment();

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

  const pick = saved[dept];
  const repo = pick && (CHAT_REPOS as readonly string[]).includes(pick) ? pick : DEFAULT_REPO;

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

  return { profile, dept, chooseDept: (name) => void choose(name), repos: CHAT_REPOS, local, repo, setRepo };
}

/** Each department's mark; one not listed falls back to the briefcase. */
const DEPT_ICONS: Record<string, LucideIcon> = {
  tech: CodeXmlIcon,
  business: BriefcaseIcon,
  sales: HandCoinsIcon,
  marketing: MegaphoneIcon,
  video: ClapperboardIcon,
  image: ImageIcon,
  support: LifeBuoyIcon,
  rnd: FlaskConicalIcon,
};
const deptIcon = (name: string): LucideIcon => DEPT_ICONS[name] ?? BriefcaseIcon;

/** GitHub's repo mark (octicon `repo`), as the hogwarts profile uses it. */
function RepoIcon({ className }: { className?: string }) {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" fill="currentColor" className={className}>
      <path d="M2 2.5A2.5 2.5 0 0 1 4.5 0h8.75a.75.75 0 0 1 .75.75v12.5a.75.75 0 0 1-.75.75h-2.5a.75.75 0 0 1 0-1.5h1.75v-2h-8a1 1 0 0 0-.714 1.7.75.75 0 1 1-1.072 1.05A2.495 2.495 0 0 1 2 11.5Zm10.5-1h-8a1 1 0 0 0-1 1v6.708A2.486 2.486 0 0 1 4.5 9h8ZM5 12.25a.25.25 0 0 1 .25-.25h3.5a.25.25 0 0 1 .25.25v3.25a.25.25 0 0 1-.4.2l-1.45-1.087a.249.249 0 0 0-.3 0L5.4 15.7a.25.25 0 0 1-.4-.2Z" />
    </svg>
  );
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
  repos: readonly string[];
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
  const [deptOpen, setDeptOpen] = useState(false);
  const [repoOpen, setRepoOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [cloning, setCloning] = useState<string | null>(null);
  const departments = profile?.departments ?? [];
  const current = departments.find((d) => d.name === dept);
  if (!current) return null;
  const title = (d: (typeof departments)[number]) => (isAr && d.title_ar) || d.title;
  const CurrentIcon = deptIcon(current.name);
  const q = query.trim().toLowerCase();
  const others = repos.filter((r) => r !== repo && (!q || r.toLowerCase().includes(q)));
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
      <Popover open={deptOpen} onOpenChange={setDeptOpen}>
        <PopoverTrigger asChild>
          <button type="button" disabled={disabled} aria-label={t(menuCopy.department)} className={trigger}>
            <CurrentIcon className="size-4 shrink-0" aria-hidden />
            <span className="truncate">{title(current)}</span>
            <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent side="bottom" align="start" dir={isAr ? "rtl" : "ltr"} className="w-64 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
          <ul>
            {departments.map((d) => {
              const Icon = deptIcon(d.name);
              return (
              <li key={d.name}>
                <button
                  type="button"
                  onClick={() => {
                    setDeptOpen(false);
                    if (d.name !== dept) onDept(d.name);
                  }}
                  className={row}
                >
                  <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
                  <span className="flex-1 truncate">{title(d)}</span>
                  {d.name === dept && <CheckIcon className="size-4 shrink-0" aria-hidden />}
                </button>
              </li>
              );
            })}
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
          <button type="button" disabled={disabled || cloning !== null} aria-label={t(menuCopy.repository)} className={trigger}>
            {cloning ? (
              <LoaderIcon className="size-4 shrink-0 animate-spin" aria-hidden />
            ) : (
              <RepoIcon className="size-4 shrink-0" />
            )}
            <span dir="ltr" className="truncate">{cloning ?? repo}</span>
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
                if (e.key === "Enter" && others[0]) pick(others[0]);
              }}
              placeholder={t(menuCopy.searchRepos)}
              className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
            />
          </label>
          <div className="mt-1.5 flex items-center gap-2.5 rounded-lg bg-muted px-2.5 py-1.5 text-sm">
            <RepoIcon className="size-4 shrink-0 text-muted-foreground" />
            <span dir="ltr" className="flex-1 truncate">{repo}</span>
            <CheckIcon className="size-4 shrink-0" aria-hidden />
          </div>
          {others.length > 0 && (
            <ul className="mt-1.5 max-h-64 overflow-y-auto border-t border-border/60 pt-1.5">
              {others.map((r) => (
                <li key={r}>
                  <button type="button" onClick={() => pick(r)} className={row}>
                    <RepoIcon className="size-4 shrink-0 text-muted-foreground" />
                    <span dir="ltr" className="flex-1 truncate">{r}</span>
                    {remote(r) && (
                      <CloudIcon className="size-4 shrink-0 text-muted-foreground" aria-label={t(menuCopy.notCloned)} />
                    )}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </PopoverContent>
      </Popover>

      {health && onAsk && <HealthLight lang={lang} dept={dept} state={health} onAsk={onAsk} />}
    </div>
  );
}
