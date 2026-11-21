"use client";

import { Suspense } from "react";
import { usePathname } from "next/navigation";

import type { Locale } from "@/components/local/config";
import { SHELL_GRID } from "./grid";
import { AppSidebar } from "./sidebar";

export { SHELL_GRID, SHELL_TOP } from "./grid";

/**
 * The page frame under the layout. Home (the chat) stays as it always was: the page alone, with the
 * mega menu as its navigation. Every other page gets the same sidebar the menu has, at the same place.
 */
export function ShellFrame({ lang, children }: { lang: Locale; children: React.ReactNode }) {
  const here = (usePathname() ?? "").replace(/\/+$/, "");
  const home = here === `/${lang}` || here === "";
  if (home) return <div className="h-full min-w-0">{children}</div>;
  return (
    <div className={`h-full lg:grid-rows-[minmax(0,1fr)] ${SHELL_GRID}`}>
      <Suspense fallback={null}>
        <AppSidebar lang={lang} />
      </Suspense>
      <div className="h-full min-w-0 overflow-hidden lg:self-stretch">{children}</div>
    </div>
  );
}
