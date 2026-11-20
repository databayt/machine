"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";

import type { Locale } from "@/components/local/config";
import { tr } from "../chat/config";
import { copy } from "../menu/config";
import { Departments } from "../menu/departments";
import { useDepartment } from "../menu/use-department";
import { readinessState } from "../readiness/actions";
import { getProfile, type MachineProfile } from "../profile/actions";
import { useInMachine } from "./content";

/**
 * The persistent app sidebar: the menu's Departments column (same component, same look) kept on
 * screen beside every page. Departments, then a few links with the current one highlighted; the
 * hamburger menu (on every screen size) carries every page. Large screens only. Outside the app
 * (a plain browser) there is no profile, so only the links show.
 */
export function AppSidebar({ lang }: { lang: Locale }): React.JSX.Element {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const inApp = useInMachine();
  const pathname = usePathname() ?? "";
  const [profile, setProfile] = useState<MachineProfile | null>(null);
  const [readyPct, setReadyPct] = useState<number | null>(null);
  const { dept, choose } = useDepartment(profile);

  useEffect(() => {
    if (!inApp) return;
    getProfile().then(setProfile).catch(() => setProfile(null));
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
  // A few links only; the hamburger menu carries every page.
  const links = [
    { href: `${base}/`, label: t(copy.home), active: is("") },
    ...(inApp ? [page("/readiness", copy.readiness, notReady)] : []),
    page("/sessions", copy.sessions),
    page("/settings", copy.settings),
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
      />
    </aside>
  );
}
