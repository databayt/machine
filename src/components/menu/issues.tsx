"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { ArrowUpRightIcon, LockIcon } from "lucide-react";

import type { Locale } from "@/components/local/config";
import type { Task } from "../tasks/types";
import { tr } from "../chat/config";
import { copy } from "./config";
import type { Readiness, TasksView } from "./data";
import { runStart } from "./data";
import { quiet } from "./ui";
import { doctorDetail, doctorNote, doctorSteps, isDoctorRefusal } from "../tasks/doctor";

export { MAX_CARDS, upcoming } from "./queue";

const taskHref = (lang: string, id: string) => `/${lang}/?task=${encodeURIComponent(id)}`;

function readinessOf(view: TasksView, t: Task): Readiness {
  return view.readiness?.[t.id] ?? { state: t.afterOpen.length ? "blocked" : "ready", missing: [], doctorExtras: 0, doctorChecking: false };
}

/** `repo:kun` → "repo kun" in words; the first missing reason is enough for a card. */
function describe(m: string, isAr: boolean): string {
  const [k, ...rest] = m.split(":");
  const v = rest.join(":");
  const en: Record<string, string> = {
    repo: `Needs the ${v} repo`,
    behind: `${v} is behind`,
    item: `Needs ${v}`,
    after: `After ${v}`,
    held: `Held by ${v}`,
    wip: "Too much in progress",
    profile: `Not for ${v}`,
    doctor: `Doctor: ${v}`,
  };
  const ar: Record<string, string> = {
    repo: `يلزم مستودع ${v}`,
    behind: `${v} متأخر`,
    item: `يلزم ${v}`,
    after: `بعد ${v}`,
    held: `بيد ${v}`,
    wip: "أعمال جارية كثيرة",
    profile: `ليست لـ ${v}`,
    doctor: `الطبيب: ${v}`,
  };
  return (isAr ? ar : en)[k] ?? m;
}

const pill = "inline-flex shrink-0 items-center rounded-full px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50";
const solid = `${pill} bg-foreground text-background hover:bg-foreground/85`;

/** Row 1: the sprint line and the upcoming issues as cards. */
export function Issues({
  lang,
  view,
  tasks,
  login,
  loading,
  onStarted,
}: {
  lang: Locale;
  view: TasksView | null;
  tasks: Task[];
  login: string | undefined;
  loading: boolean;
  onStarted: () => void;
}): React.JSX.Element | null {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const router = useRouter();
  const [busy, setBusy] = useState<string | null>(null);
  // The server said no to a claim the gate let through: the doctor's refusal gets its own line.
  const [refused, setRefused] = useState<{ id: string; doctor: boolean; text: string } | null>(null);

  async function start(id: string) {
    setBusy(id);
    setRefused(null);
    try {
      await runStart(id);
      onStarted();
      router.push(taskHref(lang, id));
    } catch (e) {
      const message = String(e);
      setRefused({ id, doctor: isDoctorRefusal(message), text: doctorDetail(message) || message });
      setBusy(null);
    }
  }

  if (!view) {
    return loading ? (
      <ul aria-hidden className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {[0, 1, 2].map((i) => (
          <li key={i} className="h-44 animate-pulse rounded-3xl bg-background" />
        ))}
      </ul>
    ) : null;
  }

  return (
    <div className="min-w-0">
      {view.sprint && (
        <p className="pb-4 text-sm leading-snug text-muted-foreground">
          <span className="font-medium">{view.sprint.title}</span>
          {view.sprint.goal ? ` — ${view.sprint.goal}` : ""}
        </p>
      )}
      {tasks.length === 0 ? (
        <p className={quiet}>{t(copy.noSprintTasks)}</p>
      ) : (
        <ul className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {tasks.map((task) => {
            const r = readinessOf(view, task);
            const other = !!task.holder && task.holder !== login;
            const mine = !!task.holder && task.holder === login;
            const repoMissing = r.missing.some((m) => m.startsWith("repo:"));
            const doctor = doctorSteps(r.missing);
            const refusal = refused?.id === task.id ? refused : null;
            const note = doctor.length === 0 ? doctorNote(r, isAr) : null;
            return (
              <li key={task.id} className="relative flex min-h-44 min-w-0 flex-col justify-between gap-4 rounded-3xl bg-background p-5">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <span dir="ltr" className="truncate">
                      {task.repo}#{task.number}
                    </span>
                    {task.priority !== null && <span>P{task.priority}</span>}
                    <a
                      href={task.url}
                      target="_blank"
                      rel="noreferrer"
                      aria-label={t(copy.openIssue)}
                      title={t(copy.openIssue)}
                      className="ms-auto -me-1 flex size-7 shrink-0 items-center justify-center rounded-full transition-colors hover:bg-foreground hover:text-background"
                    >
                      <ArrowUpRightIcon aria-hidden className="size-4 rtl:-scale-x-100" />
                    </a>
                  </div>
                  <h3 dir="auto" className="line-clamp-2 pt-2 text-start text-xl font-semibold leading-snug tracking-tight">
                    {task.title}
                  </h3>
                </div>
                <div className="flex min-w-0 items-center gap-2 text-sm text-muted-foreground">
                  {other ? (
                    <span className="truncate">{task.holder}</span>
                  ) : mine ? (
                    <Link href={taskHref(lang, task.id)} className={solid}>
                      {t(copy.continue)}
                    </Link>
                  ) : doctor.length > 0 ? (
                    <>
                      <LockIcon aria-hidden className="size-3.5 shrink-0" />
                      <span className="truncate">{t(copy.doctorFirst)}</span>
                      <Link href={`/${lang}/setup/`} className="shrink-0 underline underline-offset-4 hover:text-foreground">
                        {t({ en: "Setup", ar: "الإعداد" })}
                      </Link>
                    </>
                  ) : r.state === "ready" ? (
                    <button type="button" disabled={busy !== null} onClick={() => void start(task.id)} className={solid}>
                      {t(copy.start)}
                    </button>
                  ) : r.state === "blocked" ? (
                    <>
                      <LockIcon aria-hidden className="size-3.5 shrink-0" />
                      <span className="truncate">
                        {task.afterOpen.length
                          ? `${t({ en: "after", ar: "بعد" })} ${task.afterOpen.map((a) => a.replace(/^.*#/, "#")).join(" ")}`
                          : r.missing.map((m) => describe(m, isAr)).join(" · ")}
                      </span>
                    </>
                  ) : (
                    <>
                      <span className="line-clamp-2">{r.missing.map((m) => describe(m, isAr)).join(" · ")}</span>
                      {repoMissing && (
                        <Link href={`/${lang}/setup/?stage=repos&step=all`} className="shrink-0 underline underline-offset-4 hover:text-foreground">
                          {t(copy.addRepo)}
                        </Link>
                      )}
                    </>
                  )}
                </div>
                {note && !other && (
                  <p className="text-xs text-muted-foreground">
                    {note.link ? (
                      <Link href={`/${lang}/setup/`} className="underline-offset-4 hover:text-foreground hover:underline">
                        {note.text}
                      </Link>
                    ) : (
                      note.text
                    )}
                  </p>
                )}
                {refusal && (
                  <p role="alert" className="text-sm text-destructive">
                    {refusal.doctor ? (
                      <>
                        {t(copy.doctorFirst)}
                        {refusal.text !== "doctor" && refusal.text ? ` (${refusal.text})` : ""}{" "}
                        <Link href={`/${lang}/setup/`} className="underline underline-offset-4">
                          {t({ en: "Setup", ar: "الإعداد" })}
                        </Link>
                      </>
                    ) : (
                      refusal.text
                    )}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
