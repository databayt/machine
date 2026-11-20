"use client";

import { usePathname } from "next/navigation";

import type { Locale } from "@/components/local/config";
import { AppSidebar } from "./sidebar";

/**
 * The page frame under the layout. Home (the chat) stays as it always was: the page alone, with the
 * departments living in the hamburger's mega menu. Every other page gets the short sidebar beside it
 * on large screens.
 */
export function ShellFrame({ lang, children }: { lang: Locale; children: React.ReactNode }) {
  const here = (usePathname() ?? "").replace(/\/+$/, "");
  const home = here === `/${lang}` || here === "";
  if (home) return <div className="h-full min-w-0">{children}</div>;
  return (
    <div className="grid h-full lg:grid-cols-[12rem_minmax(0,1fr)] lg:gap-x-28 lg:ps-[46px]">
      <AppSidebar lang={lang} />
      <div className="h-full min-w-0 overflow-y-auto overscroll-contain">{children}</div>
    </div>
  );
}
