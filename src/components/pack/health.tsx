"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { CheckIcon, CircleAlertIcon, LoaderIcon, XIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { health as readHealth, healthFix } from "./actions";
import type { Health, HealthLevel, HealthRow } from "./types";

const EVERY_MS = 10 * 60_000;
const FOCUS_GAP_MS = 2 * 60_000;

export interface DeptHealth {
  health: Health | null;
  checking: boolean;
  /** True while the fix runs (it can take minutes). */
  fixing: boolean;
  recheck: () => Promise<void>;
  fix: () => Promise<void>;
  /** True when the last result came from a person's tap, not the timer. */
  manual: boolean;
}

/**
 * The department's health: checked on mount, when the department changes, every 10 minutes and
 * when the window regains focus (at most every 2 minutes).
 */
export function useDeptHealth(dept: string): DeptHealth {
  const [health, setHealth] = useState<Health | null>(null);
  const [checking, setChecking] = useState(false);
  const [fixing, setFixing] = useState(false);
  const [manual, setManual] = useState(false);
  const run = useRef(0);
  const last = useRef(0);

  const go = useCallback(
    async (isManual: boolean, fn: (d: string) => Promise<Health | null>) => {
      if (!dept) return;
      const id = ++run.current;
      last.current = Date.now();
      setChecking(true);
      let next: Health | null = null;
      try {
        next = await fn(dept);
      } catch {
        next = null;
      }
      if (id !== run.current) return; // the department changed meanwhile
      setHealth(next);
      setManual(isManual);
      setChecking(false);
    },
    [dept],
  );

  useEffect(() => {
    setHealth(null);
    if (!dept) return;
    void go(false, readHealth);
    const timer = setInterval(() => void go(false, readHealth), EVERY_MS);
    const onFocus = () => {
      if (Date.now() - last.current >= FOCUS_GAP_MS) void go(false, readHealth);
    };
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [dept, go]);

  const recheck = useCallback(() => go(true, readHealth), [go]);
  const fix = useCallback(async () => {
    setFixing(true);
    try {
      await go(true, healthFix);
    } finally {
      setFixing(false);
    }
  }, [go]);

  return { health, checking, fixing, recheck, fix, manual };
}

const t = (isAr: boolean, ar: string, en: string) => (isAr ? ar : en);

const DOT: Record<Exclude<HealthLevel, "none">, string> = {
  green: "bg-emerald-500",
  orange: "bg-amber-500",
  red: "bg-red-500",
};

function label(isAr: boolean, level: HealthLevel, percent: number): string {
  if (level === "green") return "100%";
  if (level === "orange") return `${t(isAr, "كافٍ", "Sufficient")} ${percent}%`;
  return t(isAr, "معطّل", "Out of order");
}

/** The agent prompt: the rows that are not ok, compact, then ask for a re-check. */
export function diagnosePrompt(dept: string, rows: HealthRow[], error: string | null): string {
  const bad = rows
    .filter((r) => r.level !== "ok")
    .map((r) => ({ name: r.name, level: r.level, critical: r.critical, detail: r.detail }));
  const list = bad.length ? bad : error ? [{ error }] : [];
  return `شخّص وأصلح جاهزية قسم ${dept}: ${JSON.stringify(list)} ثم أعد الفحص.`;
}

const RANK: Record<string, number> = { fail: 0, warn: 1, ok: 2 };

/**
 * A small dot and a short label; tapping it opens the diagnosis. The dialog also opens once by
 * itself when an automatic check lands on orange or red (again only when the level changes).
 */
export function HealthLight({
  lang,
  dept,
  state,
  onAsk,
}: {
  lang: Locale;
  dept: string;
  state: DeptHealth;
  /** Sends a prompt to the agent, as the home suggestions do. */
  onAsk: (prompt: string) => void;
}) {
  const isAr = lang === "ar";
  const { health, checking, fixing, recheck, fix, manual } = state;
  const [open, setOpen] = useState(false);
  const told = useRef<{ dept: string; level: HealthLevel } | null>(null);

  const level = health?.level ?? "none";
  useEffect(() => {
    if (!health) return;
    const same = told.current?.dept === dept && told.current.level === health.level;
    if (!manual && !same && (health.level === "orange" || health.level === "red")) setOpen(true);
    told.current = { dept, level: health.level };
  }, [health, manual, dept]);

  if (!health || level === "none") return null;

  const text = label(isAr, level, health.percent);
  const aria = `${t(isAr, "جاهزية القسم", "Department health")}: ${text}`;
  const rows = [...health.rows].sort((a, b) => (RANK[a.level] ?? 3) - (RANK[b.level] ?? 3));
  const verdict =
    level === "green"
      ? t(isAr, "كل شيء جاهز.", "Everything is ready.")
      : level === "orange"
        ? t(isAr, `الجاهزية كافية (${health.percent}%) لكن بعض البنود تحتاج انتباهًا.`, `Sufficient (${health.percent}%), but a few items need attention.`)
        : health.error
          ? t(isAr, "تعذّر الفحص، القسم معطّل.", "The check could not run: out of order.")
          : t(isAr, "بند أساسي معطّل، القسم لا يعمل بعد.", "A critical item is down: the department cannot work yet.");
  const busy = checking || fixing;

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        title={aria}
        aria-label={aria}
        className="inline-flex min-w-0 items-center gap-1.5 rounded-lg px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
      >
        <span
          aria-hidden
          className={cn("size-2.5 shrink-0 rounded-full", checking ? "animate-pulse bg-muted-foreground/50" : DOT[level])}
        />
        <span className="truncate">{text}</span>
      </button>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent dir={isAr ? "rtl" : "ltr"}>
          <DialogTitle className="sr-only">{aria}</DialogTitle>
          <DialogDescription className="flex items-center gap-2 text-sm text-foreground">
            <span aria-hidden className={cn("size-2.5 shrink-0 rounded-full", DOT[level])} />
            {verdict}
          </DialogDescription>

          <ul className="mt-4 max-h-72 space-y-2 overflow-y-auto">
            {health.error && (
              <li dir="auto" className="font-mono text-xs text-red-600 dark:text-red-400">
                {health.error}
              </li>
            )}
            {rows.map((r) => (
              <li key={r.name} className="flex items-start gap-2.5 text-sm">
                {r.level === "ok" ? (
                  <CheckIcon className="mt-0.5 size-4 shrink-0 text-emerald-500" aria-hidden />
                ) : r.level === "warn" ? (
                  <CircleAlertIcon className="mt-0.5 size-4 shrink-0 text-amber-500" aria-hidden />
                ) : (
                  <XIcon className="mt-0.5 size-4 shrink-0 text-red-500" aria-hidden />
                )}
                <div className="min-w-0 flex-1">
                  <div className="truncate">{r.name}</div>
                  {r.detail && (
                    <div dir="auto" className="break-words font-mono text-xs text-muted-foreground">
                      {r.detail}
                    </div>
                  )}
                </div>
              </li>
            ))}
          </ul>

          <div className="mt-5 flex flex-wrap items-center gap-2">
            <Button size="sm" disabled={busy} onClick={() => void fix()}>
              {fixing && <LoaderIcon className="animate-spin" aria-hidden />}
              {t(isAr, "إصلاح تلقائي", "Fix automatically")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => {
                setOpen(false);
                onAsk(diagnosePrompt(dept, health.rows, health.error));
              }}
            >
              {t(isAr, "شخّص مع الوكيل", "Diagnose with agent")}
            </Button>
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => void recheck()}>
              {checking && !fixing && <LoaderIcon className="animate-spin" aria-hidden />}
              {t(isAr, "أعد الفحص", "Re-check")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
