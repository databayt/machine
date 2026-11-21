"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";

import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { tr } from "../chat/config";
import { copy, promptHref } from "../menu/config";
import { loadHealth, type HealthReport } from "../menu/data";
import { NavLinks } from "../menu/departments";
import { Tools } from "../menu/tools";
import { readinessState } from "../readiness/actions";
import { getProfile, type MachineProfile } from "../profile/actions";
import { useInMachine } from "./content";
import { SHELL_TOP } from "./grid";
import { inSettings, splitPath } from "./tabs";

/**
 * The one sidebar of the app, in the menu's item style: Sessions, Catalog and Settings (with
 * the readiness %), then Docs and Mathematics, then the tools row. No departments: those live in
 * Settings. The mega menu's first column renders this same component, and both sit on the shared
 * shell grid, so opening or closing the menu never moves it. Large screens only; below `lg` the
 * menu's footer carries the same links.
 */
export function AppSidebar({ lang, className }: { lang: Locale; className?: string }): React.JSX.Element {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const inApp = useInMachine();
  const pathname = usePathname() ?? "";
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [health, setHealth] = useState<HealthReport | null>(null);
  const [readyPct, setReadyPct] = useState<number | null>(null);

  useEffect(() => {
    if (!inApp) return;
    getProfile().then(setProfile).catch(() => setProfile(null));
    loadHealth().then(setHealth);
    readinessState().then((v) => setReadyPct(v && v.hasState ? Math.round(v.percent) : null)).catch(() => setReadyPct(null));
  }, [inApp]);

  const base = `/${lang}`;
  const is = (path: string) => {
    const here = pathname.replace(/\/+$/, "");
    return path === "" ? here === base : here === base + path || here.startsWith(`${base}${path}/`);
  };
  const page = (path: string, label: { en: string; ar: string }, suffix = "") => ({
    href: `${base}${path}/`,
    label: t(label) + suffix,
    active: is(path),
  });

  const notReady = readyPct !== null && readyPct < 100 ? ` · ${readyPct}%` : "";
  // Three parents (no Home: closing the menu returns to the chat); the machine pages are tabs inside Settings.
  const parents = [
    page("/sessions", copy.sessions),
    page("/catalog", copy.catalog),
    { ...page("/settings", copy.settings, notReady), active: inSettings(splitPath(pathname).rest) },
  ];
  const more = [
    { href: `https://kun.databayt.org/${lang}/docs`, label: t(copy.docs) },
    { href: promptHref(lang, t(copy.mathematicsStarter)), label: t(copy.mathematics) },
  ];
  const showEngine = inApp === true && (!profile || profile.access.engine);

  return (
    <aside className={cn("hidden min-h-0 min-w-0 overflow-y-auto overscroll-contain lg:block lg:self-stretch", SHELL_TOP, className)}>
      <nav aria-label={t(copy.title)} className="lg:w-36">
        <NavLinks links={parents} />
        <hr className="my-4 border-foreground/10" />
        <NavLinks links={more} />
        <hr className="my-4 border-foreground/10" />
        <Tools lang={lang} inApp={inApp === true} showEngine={showEngine} health={health} />
      </nav>
    </aside>
  );
}
