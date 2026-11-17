"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { usePathname } from "next/navigation";

import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import type { HealthReport } from "./data";
import { tr } from "../chat/config";
import { useInMachine } from "../shell/content";
import { getProfile, type MachineProfile } from "../profile/actions";
import { copy, DEPT_KEY } from "./config";
import { loadHealth, loadTasks, type TasksView } from "./data";
import { Departments } from "./departments";
import { Footer } from "./footer";
import { MorphButton } from "./morph-button";
import { Issues, upcoming } from "./issues";
import { Banner } from "./banner";
import { rankSpotlight } from "./spotlight";
import { workflowsFor, Workflows } from "./workflows";
import { quiet } from "./ui";

const MS = 200;
const reduced = () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * The machine's mega menu. A fixed two-line button morphs into an X; the full-screen dialog fades and
 * lifts in. Departments on the first column; the rest is the work area: the banner, the department's workflows as a carousel, then the upcoming issues as cards. A footer strip below.
 */
export function MachineMenu({ lang }: { lang: Locale }) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const inApp = useInMachine();
  const pathname = usePathname();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const firstDept = useRef<HTMLButtonElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const [mounted, setMounted] = useState(false);
  const [visible, setVisible] = useState(false);
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [dept, setDept] = useState<string>("");
  const [tasks, setTasks] = useState<TasksView | null>(null);
  const [tasksLoading, setTasksLoading] = useState(false);
  const [health, setHealth] = useState<HealthReport | null>(null);
  const [unread, setUnread] = useState(0);

  const open = useCallback(() => {
    clearTimeout(timer.current);
    setMounted(true);
    document.documentElement.style.overflow = "hidden";
    requestAnimationFrame(() => requestAnimationFrame(() => setVisible(true)));
  }, []);
  const close = useCallback(() => {
    setVisible(false);
    clearTimeout(timer.current);
    const done = () => {
      setMounted(false);
      document.documentElement.style.overflow = "";
    };
    if (reduced()) done();
    else timer.current = setTimeout(done, MS);
    trigger.current?.focus();
  }, []);
  useEffect(() => () => clearTimeout(timer.current), []);

  // Navigating closes it.
  useEffect(() => {
    if (mounted) close();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathname]);

  // The profile once; everything else each time it opens.
  useEffect(() => {
    if (!inApp) return;
    getProfile().then(setProfile).catch(() => setProfile(null));
  }, [inApp]);

  // Default = remembered choice if the profile still has it, else its first department.
  useEffect(() => {
    if (!profile?.departments.length) return;
    let saved = "";
    try {
      saved = localStorage.getItem(DEPT_KEY) ?? "";
    } catch {}
    const names = profile.departments.map((d) => d.name);
    setDept(names.includes(saved) ? saved : names[0]);
  }, [profile]);

  const choose = (name: string) => {
    setDept(name);
    try {
      localStorage.setItem(DEPT_KEY, name);
    } catch {}
  };

  useEffect(() => {
    if (!visible || !inApp) return;
    loadHealth().then(setHealth);
  }, [visible, inApp]);

  useEffect(() => {
    if (!visible || !inApp || !dept) return;
    let live = true;
    setTasks(null);
    setTasksLoading(true);
    loadTasks(dept).then((v) => {
      if (!live) return;
      setTasks(v);
      setTasksLoading(false);
    });
    return () => {
      live = false;
    };
  }, [visible, inApp, dept]);

  // Unread CEO notes: the network plugin emits `network:unread` after each heartbeat.
  useEffect(() => {
    if (!inApp) return;
    let off: (() => void) | undefined;
    let live = true;
    import("@tauri-apps/api/event")
      .then((m) => m.listen<{ count: number }>("network:unread", (e) => setUnread(e.payload?.count ?? 0)))
      .then((f) => (live ? (off = f) : f()))
      .catch(() => {});
    return () => {
      live = false;
      off?.();
    };
  }, [inApp]);

  // Focus into the dialog on open.
  useEffect(() => {
    if (visible) firstDept.current?.focus({ preventScroll: true });
  }, [visible, dept, profile]);

  // Esc closes; Tab stays inside the dialog and the button that closes it.
  useEffect(() => {
    if (!mounted) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") return close();
      if (e.key !== "Tab" || !root.current) return;
      const items = [
        trigger.current,
        ...root.current.querySelectorAll<HTMLElement>("a[href],button:not([disabled])"),
      ].filter((x): x is HTMLElement => !!x);
      const i = items.indexOf(document.activeElement as HTMLElement);
      const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : i === items.length - 1 ? 0 : i + 1;
      e.preventDefault();
      items[next]?.focus();
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, [mounted, close]);

  const showCatalog = !profile || profile.access.catalog === "all" || profile.access.catalog === "profile";
  const showEngine = inApp === true && (!profile || profile.access.engine);
  const current = profile?.departments.find((d) => d.name === dept);
  const queue = useMemo(() => upcoming(tasks, profile?.login), [tasks, profile?.login]);
  const workflows = useMemo(
    () => workflowsFor({ lang, dept: current, hidden: profile?.hidden ?? [] }),
    [lang, current, profile?.hidden],
  );
  const spotlight = useMemo(
    () =>
      rankSpotlight({
        lang,
        view: tasks,
        tasks: queue,
        login: profile?.login,
        unread,
        health,
        lanes: (current?.quick_actions ?? []).map((q) => ({
          label: lang === "ar" ? q.label_ar : q.label_en,
          prompt: q.prompt,
          detail: (lang === "ar" ? q.description_ar : q.description_en) || "",
        })),
        department: (lang === "ar" && current?.title_ar) || current?.title || "",
      }),
    [lang, tasks, queue, profile?.login, unread, health, current],
  );

  return (
    <>
      <MorphButton
        buttonRef={trigger}
        open={visible}
        label={t(visible ? copy.close : copy.open)}
        onClick={visible ? close : open}
      />
      {mounted && (
        <div
          ref={root}
          role="dialog"
          aria-modal="true"
          aria-label={t(copy.title)}
          dir={isAr ? "rtl" : "ltr"}
          onClick={(e) => {
            const a = (e.target as HTMLElement).closest("a");
            if (a && a.target !== "_blank") close();
          }}
          className={cn(
            "fixed inset-0 z-50 flex h-dvh flex-col overflow-y-auto overscroll-contain bg-muted text-foreground transition-[opacity,transform] duration-200 ease-out motion-reduce:transition-none",
            visible ? "translate-y-0 opacity-100" : "translate-y-2 opacity-0",
          )}
        >
          <nav
            aria-label={t(copy.title)}
            className="flex w-full flex-1 flex-col justify-center gap-8 ps-[26px] pe-6 pb-4 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:grid lg:grid-cols-4 lg:content-center lg:items-center lg:gap-x-10 lg:py-16"
          >
            {inApp && profile && profile.departments.length > 0 ? (
              <>
                <Departments
                  isAr={isAr}
                  profile={profile}
                  selected={dept}
                  label={t(copy.title)}
                  onSelect={choose}
                  first={firstDept}
                  links={[
                    { href: `/${lang}/settings/`, label: t(copy.settings) },
                    ...(showCatalog ? [{ href: `/${lang}/catalog/`, label: t(copy.catalog) }] : []),
                    { href: `/${lang}/sessions/`, label: t(copy.sessions) },
                  ]}
                />
                <div className="flex min-w-0 flex-col gap-8 lg:col-span-3">
                  <Banner items={spotlight} lang={lang} onNavigate={close} />
                  <Workflows lang={lang} workflows={workflows} />
                  <Issues lang={lang} view={tasks} tasks={queue} login={profile.login} loading={tasksLoading} onStarted={close} />
                </div>
              </>
            ) : (
              <p className={cn(quiet, "lg:col-span-4 lg:text-center")}>
                {inApp ? t(copy.noDept) : t(copy.appOnly)}
              </p>
            )}
          </nav>
          <Footer lang={lang} inApp={inApp === true} showCatalog={showCatalog} showEngine={showEngine} health={health} />
        </div>
      )}
    </>
  );
}
