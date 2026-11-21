"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { usePathname, useRouter } from "next/navigation";

import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import type { HealthReport } from "./data";
import { tr } from "../chat/config";
import { useInMachine } from "../shell/content";
import { SHELL_GRID } from "../shell/grid";
import { AppSidebar } from "../shell/sidebar";
import { getProfile, type MachineProfile } from "../profile/actions";
import { copy } from "./config";
import { useDepartment } from "./use-department";
import { loadHealth, loadTasks, type TasksView } from "./data";
import { Footer } from "./footer";
import { Keywords } from "./keywords";
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
 * lifts in. The first column is the app's sidebar (the same component every page has); the rest is the work area: the banner, the department's workflows as a carousel, then the upcoming issues as cards. A footer strip below.
 */
export function MachineMenu({ lang }: { lang: Locale }) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const inApp = useInMachine();
  const pathname = usePathname();
  const router = useRouter();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const [mounted, setMounted] = useState(false);
  const [visible, setVisible] = useState(false);
  const [profile, setProfile] = useState<MachineProfile | null>(null);
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
  /**
   * `home`: the X button and Esc, which return to the chat unless already there. A link click or a
   * pathname change closes without redirecting.
   */
  const atHome = (pathname ?? "").replace(/\/+$/, "") === `/${lang}` || !pathname;
  const homeRef = useRef(atHome);
  homeRef.current = atHome;
  const close = useCallback(
    (opts?: { home?: boolean }) => {
    if (opts?.home && !homeRef.current) router.push(`/${lang}/`);
    setVisible(false);
    clearTimeout(timer.current);
    const done = () => {
      setMounted(false);
      document.documentElement.style.overflow = "";
    };
    if (reduced()) done();
    else timer.current = setTimeout(done, MS);
    trigger.current?.focus();
    },
    [router, lang],
  );
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

  const { dept } = useDepartment();

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

  // Focus into the dialog on open — once, onto the dialog itself, so no item is lit.
  useEffect(() => {
    if (visible) root.current?.focus({ preventScroll: true });
  }, [visible]);

  // Esc closes; Tab stays inside the dialog and the button that closes it.
  useEffect(() => {
    if (!mounted) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") return close({ home: true });
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
        onClick={visible ? () => close({ home: true }) : open}
      />
      {mounted && (
        <div
          ref={root}
          role="dialog"
          aria-modal="true"
          aria-label={t(copy.title)}
          dir={isAr ? "rtl" : "ltr"}
          tabIndex={-1}
          onClick={(e) => {
            const a = (e.target as HTMLElement).closest("a");
            if (a && a.target !== "_blank") close();
          }}
          className={cn(
            "fixed inset-0 z-50 flex h-dvh outline-none flex-col overflow-y-auto overscroll-contain bg-muted text-foreground transition-[opacity,transform] duration-200 ease-out motion-reduce:transition-none",
            visible ? "translate-y-0 opacity-100" : "translate-y-2 opacity-0",
          )}
        >
          <nav
            aria-label={t(copy.title)}
            className={cn(
              "flex w-full flex-1 flex-col justify-center gap-8 ps-[26px] pe-6 pb-4 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:pe-16",
              SHELL_GRID,
              // The menu keeps its own vertical place (the block centred, as it always was); inside it
              // the sidebar starts level with the work area's first line instead of floating mid-column.
              "lg:content-center lg:items-start lg:py-32",
            )}
          >
            <AppSidebar lang={lang} className="lg:self-auto lg:pt-0" />
            {inApp && profile && profile.departments.length > 0 ? (
              <div className="flex min-w-0 flex-col gap-10">
                <Banner items={spotlight} lang={lang} onNavigate={() => close()} />
                <Workflows lang={lang} workflows={workflows} />
                <Keywords lang={lang} department={current} />
                <Issues lang={lang} view={tasks} tasks={queue} login={profile.login} loading={tasksLoading} onStarted={() => close()} />
              </div>
            ) : (
              <p className={quiet}>{inApp ? t(copy.noDept) : t(copy.appOnly)}</p>
            )}
          </nav>
          <Footer lang={lang} inApp={inApp === true} showEngine={showEngine} health={health} />
        </div>
      )}
    </>
  );
}
