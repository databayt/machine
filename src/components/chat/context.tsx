"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { BookIcon, BriefcaseIcon, CheckIcon, ChevronDownIcon } from "lucide-react";

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
  repos: string[];
  repo: string;
  setRepo: (repo: string) => void;
} {
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [local, setLocal] = useState<Set<string> | null>(null);
  const [saved, setSaved] = useState<Record<string, string>>({});
  const { dept } = useDepartment();

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

  return { profile, dept, repos, repo, setRepo };
}

/** `▣ Department ▾   ▤ Repository ▾`, one quiet row under the composer. */
export function ContextLine({
  lang,
  profile,
  dept,
  repos,
  repo,
  onRepo,
  disabled,
}: {
  lang: Locale;
  profile: MachineProfile | null;
  dept: string;
  repos: string[];
  repo: string;
  onRepo: (repo: string) => void;
  disabled?: boolean;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const [open, setOpen] = useState(false);
  const current = profile?.departments.find((d) => d.name === dept);
  if (!current) return null;
  const title = (isAr && current.title_ar) || current.title;
  const row = "flex w-full items-center justify-between gap-3 rounded-xl px-3 py-2 text-start text-sm transition-colors hover:bg-muted";

  return (
    <div className="mt-2.5 flex min-w-0 items-center gap-5 px-3 text-sm text-foreground/80">
      <Link href={`/${lang}/settings/`} className="inline-flex min-w-0 items-center gap-1.5 transition-colors hover:text-foreground">
        <BriefcaseIcon className="size-4 shrink-0" aria-hidden />
        <span className="truncate">{title}</span>
        <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
      </Link>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            disabled={disabled}
            aria-label={t(menuCopy.repository)}
            className="inline-flex min-w-0 items-center gap-1.5 transition-colors hover:text-foreground disabled:opacity-50"
          >
            <BookIcon className="size-4 shrink-0" aria-hidden />
            <span dir="ltr" className="truncate">{repo || t(menuCopy.allRepos)}</span>
            <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent side="bottom" align="start" dir={isAr ? "rtl" : "ltr"} className="w-64 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
          <ul>
            {["", ...repos].map((r) => (
              <li key={r || "all"}>
                <button
                  type="button"
                  onClick={() => {
                    setOpen(false);
                    onRepo(r);
                  }}
                  className={cn(row, r === repo && "text-foreground")}
                >
                  <span dir="ltr" className="truncate">{r || t(menuCopy.allRepos)}</span>
                  {r === repo && <CheckIcon className="size-4 shrink-0" aria-hidden />}
                </button>
              </li>
            ))}
          </ul>
          {repos.length === 0 && <p className="px-3 pb-2 pt-1 text-xs">{t(menuCopy.noRepoHere)}</p>}
        </PopoverContent>
      </Popover>
    </div>
  );
}
