"use client";

import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { tr } from "../chat/config";
import { copy, promptHref } from "../menu/config";
import { loadHealth, type HealthReport } from "../menu/data";
import { NavLinks, type NavLink } from "../menu/departments";
import { Tools } from "../menu/tools";
import { useEffect, useState } from "react";
import { SHELL_TOP } from "./grid";
import { itemName, type NavNode } from "./nav";
import { useNav } from "./use-nav";

/**
 * The one sidebar of the app, in the menu's item style: a tree of four parents (Sessions, Install,
 * .claude, Settings), then Docs and Mathematics, then the tools row. The active parent opens its
 * children under it and the deepest open level opens one more line, so the sidebar doubles as the
 * breadcrumb; every line above the deepest is a link back up. One white mark, on the deepest listed
 * line. The mega menu's first column renders this same component on the same shell grid. It reads the
 * address (`useSearchParams`), so it needs a `<Suspense>` above it. Large screens only; below `lg` the
 * menu's footer carries the four parents.
 */
export function AppSidebar({ lang, className }: { lang: Locale; className?: string }): React.JSX.Element {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const { inApp, engine, view, readyPct, tree, trail } = useNav(lang);
  const [health, setHealth] = useState<HealthReport | null>(null);

  useEffect(() => {
    if (inApp) loadHealth().then(setHealth);
  }, [inApp]);

  const text = (n: NavNode) => (typeof n.label === "string" ? n.label : t(n.label));
  const notReady = readyPct !== null && readyPct < 100 ? ` · ${readyPct}%` : "";

  /** The line under the active child, naming the open step or item. */
  const leafLink = (parent: NavNode, child: NavNode): NavLink | null => {
    const leaf = trail.leaf;
    if (!leaf) return null;
    let label: string;
    let href: string;
    if (parent.id === "install") {
      const step = view?.steps.find((s) => s.id === leaf);
      label =
        child.id === "repos" && leaf === "all"
          ? t(copy.allRepositories)
          : child.id === "agents" && leaf === "default"
            ? t(copy.defaultAgent)
            : (step && (isAr ? step.title.ar : step.title.en)) || leaf;
      href = `${child.href(lang)}&step=${encodeURIComponent(leaf)}`;
    } else {
      label = itemName(leaf);
      href = `${child.href(lang)}&item=${encodeURIComponent(leaf)}`;
    }
    return { href, label, active: true, mark: true, current: true, ltr: parent.id === "claude" };
  };

  const parents: NavLink[] = tree.map((node) => {
    const open = trail.parent === node.id;
    let kids = node.children ?? [];
    // An open stage the catalog does not list (yet) still shows, by its id.
    if (open && node.id === "install" && trail.child && !kids.some((k) => k.id === trail.child)) {
      const id = trail.child;
      kids = [...kids, { id, label: id, href: (l) => `/${l}/install/?stage=${encodeURIComponent(id)}` }];
    }
    const children: NavLink[] | undefined = open
      ? kids.map((k) => {
          const on = trail.child === k.id;
          const leaf = on ? leafLink(node, k) : null;
          return {
            href: k.href(lang),
            label: text(k),
            hint: k.hint,
            ltr: node.id === "claude",
            active: on,
            mark: on && !leaf,
            current: on && !leaf,
            children: leaf ? [leaf] : undefined,
          };
        })
      : undefined;
    const here = open && !trail.child;
    return {
      href: node.href(lang),
      label: text(node) + (node.id === "install" ? notReady : ""),
      active: open,
      mark: here,
      current: here,
      ltr: node.id === "claude",
      children,
    };
  });

  const more = [
    { href: `https://kun.databayt.org/${lang}/docs`, label: t(copy.docs) },
    { href: promptHref(lang, t(copy.mathematicsStarter)), label: t(copy.mathematics) },
  ];
  const showEngine = inApp === true && engine;

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
