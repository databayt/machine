"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { CheckIcon, ChevronDownIcon } from "lucide-react";

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
 */
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
  const repos = (current?.repos ?? [])
    .map((r) => r.name)
    .filter((n) => local?.has(n) && (listed.length === 0 || listed.includes(n)));
  const pick = saved[dept] ?? "";
  const repo = repos.includes(pick) ? pick : "";

  const setRepo = useCallback(
    (next: string) => {
      setSaved((all) => ({ ...all, [dept]: next }));
      void (async () => {
        const now = await loadSettings();
        if (!now) return;
        const map = { ...now.repo_by_department };
        if (next) map[dept] = next;
        else delete map[dept];
        await saveSettings({ ...now, repo_by_department: map }).catch(() => {});
      })();
    },
    [dept],
  );

  return { profile, dept, repos, repo, setRepo };
}

/** `Department · Repository ▾`, one quiet line above the composer. */
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
    <div className="mb-2 flex min-w-0 items-center gap-1.5 px-4 text-sm text-muted-foreground">
      <Link href={`/${lang}/settings/`} className="truncate transition-colors hover:text-foreground">
        {title}
      </Link>
      <span aria-hidden>·</span>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            disabled={disabled}
            aria-label={t(menuCopy.repository)}
            className="inline-flex min-w-0 items-center gap-1 transition-colors hover:text-foreground disabled:opacity-50"
          >
            <span dir="ltr" className="truncate">{repo || t(menuCopy.allRepos)}</span>
            <ChevronDownIcon className="size-3.5 shrink-0" aria-hidden />
          </button>
        </PopoverTrigger>
        <PopoverContent side="top" align="start" dir={isAr ? "rtl" : "ltr"} className="w-64 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
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
