"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";

import type { Locale } from "@/components/local/config";
import { tr } from "../chat/config";
import { copy } from "../menu/config";
import { loadHealth, type HealthReport } from "../menu/data";
import { Departments } from "../menu/departments";
import { Tools } from "../menu/tools";
import { useDepartment } from "../menu/use-department";
import { readinessState } from "../readiness/actions";
import { getProfile, type MachineProfile } from "../profile/actions";
import { useInMachine } from "./content";

/**
 * The persistent app sidebar: the menu's Departments column (same component, same look) kept on
 * screen beside every page. Departments, then a link to each page with the current one highlighted,
 * then the tools. Large screens only; below `lg` the mega menu is the way around. Outside the app
 * (a plain browser) there is no profile, so only the links show.
 */
export function AppSidebar({ lang }: { lang: Locale }): React.JSX.Element {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const inApp = useInMachine();
  const pathname = usePathname() ?? "";
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [health, setHealth] = useState<HealthReport | null>(null);
  const [readyPct, setReadyPct] = useState<number | null>(null);
  const { dept, choose } = useDepartment(profile);

  useEffect(() => {
    if (!inApp) return;
    getProfile().then(setProfile).catch(() => setProfile(null));
    loadHealth().then(setHealth);
    readinessState().then((v) => setReadyPct(v && v.hasState ? Math.round(v.percent) : null)).catch(() => setReadyPct(null));
  }, [inApp]);

  const showEngine = inApp === true && (!profile || profile.access.engine);
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
  const links = [
    { href: `${base}/`, label: t(copy.home), active: is("") },
    ...(inApp ? [page("/readiness", copy.readiness, notReady)] : []),
    page("/sessions", copy.sessions),
    ...(inApp ? [page("/repos", copy.repositories)] : []),
    ...(inApp ? [page("/agents", copy.agents)] : []),
    page("/catalog", copy.catalog),
    ...(inApp ? [page("/config", copy.configuration)] : []),
    ...(inApp ? [page("/health", copy.healthUpdates)] : []),
    ...(showEngine ? [page("/engine", copy.engine)] : []),
    page("/settings", copy.settings),
    { href: `https://kun.databayt.org/${lang}/docs`, label: t(copy.docs) },
  ];

  return (
    <aside className="hidden min-h-0 min-w-0 overflow-y-auto overscroll-contain lg:block lg:pt-16">
      <Departments
        isAr={isAr}
        profile={inApp && profile ? profile : { departments: [] }}
        selected={dept}
        label={t(copy.title)}
        onSelect={choose}
        links={links}
        tools={inApp ? <Tools lang={lang} inApp showEngine={showEngine} health={health} /> : undefined}
      />
    </aside>
  );
}
