"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";

import { cn } from "@/lib/utils";
import { tr } from "../chat/config";
import { copy } from "../menu/config";
import { getProfile } from "../profile/actions";
import { useInMachine } from "./content";

/** Settings is the parent; each machine page is a tab inside it. Order is the tab order. */
export const SETTINGS_TABS = [
  { path: "/settings", label: copy.general },
  { path: "/readiness", label: copy.readiness, app: true },
  { path: "/repos", label: copy.repositories, app: true },
  { path: "/agents", label: copy.agents, app: true },
  { path: "/config", label: copy.configuration, app: true },
  { path: "/health", label: copy.healthUpdates, app: true },
  { path: "/engine", label: copy.engine, app: true, engine: true },
] as const;

/** `/ar/agents/` → { lang: "ar", rest: "/agents" } */
export function splitPath(pathname: string): { lang: string; rest: string } {
  const [, lang = "", ...more] = pathname.replace(/\/+$/, "").split("/");
  return { lang, rest: more.length ? `/${more.join("/")}` : "" };
}

export function inSettings(rest: string): boolean {
  return SETTINGS_TABS.some((t) => rest === t.path || rest.startsWith(`${t.path}/`));
}

/** The tab row on top of every Settings page; renders nothing elsewhere. */
export function SettingsTabs() {
  const { lang, rest } = splitPath(usePathname() ?? "");
  const inApp = useInMachine();
  const [engine, setEngine] = useState(false);
  useEffect(() => {
    if (!inApp) return;
    getProfile()
      .then((p) => setEngine(!p || p.access.engine))
      .catch(() => setEngine(false));
  }, [inApp]);
  if (!inSettings(rest)) return null;
  const t = tr(lang === "ar");
  const tabs = SETTINGS_TABS.filter((x) => (!("app" in x) || inApp) && (!("engine" in x) || engine));
  return (
    <nav aria-label={t(copy.settings)} className="-mx-3 flex min-w-0 flex-wrap gap-1">
      {tabs.map((x) => {
        const active = rest === x.path || rest.startsWith(`${x.path}/`);
        return (
          <Link
            key={x.path}
            href={`/${lang}${x.path}/`}
            aria-current={active ? "page" : undefined}
            className={cn(
              "rounded-full px-3 py-1.5 text-sm transition-colors",
              active ? "bg-background text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {t(x.label)}
          </Link>
        );
      })}
    </nav>
  );
}
