"use client";

import { useState } from "react";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";

import Link from "next/link";

import { cn } from "@/lib/utils";
import type { MachineProfile } from "../profile/types";

/**
 * Column 1: the departments at the top, then a divider and the app links at the foot, so on large
 * screens the column is as tall as the work area beside it; a scroll row of departments on phones.
 *
 * The highlight is hogwarts' homepage one (`components/atom/card-hover-effect.tsx`): one shared
 * `layoutId` background that slides to the item under the pointer or keyboard focus and rests on
 * the selected department — white here, on the menu's gray. The button pulls itself back by its own
 * padding (`-ms-4 px-4`) so the names still start on the close mark's line.
 */
export function Departments({
  isAr,
  profile,
  selected,
  label,
  onSelect,
  first,
  links = [],
}: {
  isAr: boolean;
  profile: MachineProfile;
  selected: string;
  label: string;
  onSelect: (name: string) => void;
  first: React.Ref<HTMLButtonElement>;
  /** Under a divider, after the departments: Settings, Catalog, Sessions… */
  links?: { href: string; label: string }[];
}): React.JSX.Element {
  const [hovered, setHovered] = useState<string | null>(null);
  const lit = hovered ?? selected;

  return (
    <section className="min-w-0 lg:flex lg:flex-col lg:justify-between lg:self-stretch">
      <LayoutGroup id="departments">
        <ul
          role="listbox"
          aria-label={label}
          onMouseLeave={() => setHovered(null)}
          className="-mx-6 flex gap-2 overflow-x-auto px-6 pb-1 lg:mx-0 lg:flex-col lg:gap-1 lg:overflow-visible lg:px-0"
        >
          {profile.departments.map((d) => {
            const on = d.name === selected;
            return (
              <li key={d.name} role="presentation" className="shrink-0">
                <button
                  ref={on ? first : undefined}
                  type="button"
                  role="option"
                  aria-selected={on}
                  onClick={() => onSelect(d.name)}
                  onMouseEnter={() => setHovered(d.name)}
                  onFocus={() => setHovered(d.name)}
                  onBlur={() => setHovered(null)}
                  className={cn(
                    "relative block whitespace-nowrap rounded-2xl px-4 py-2.5 text-start text-lg font-semibold leading-tight tracking-tight outline-none transition-colors lg:-ms-4 lg:text-2xl",
                    on ? "text-foreground" : "text-muted-foreground hover:text-foreground focus-visible:text-foreground",
                  )}
                >
                  <AnimatePresence>
                    {lit === d.name && (
                      <motion.span
                        aria-hidden
                        layoutId="department-highlight"
                        className="absolute inset-0 -z-0 rounded-2xl bg-background"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1, transition: { duration: 0.15 } }}
                        exit={{ opacity: 0, transition: { duration: 0.15, delay: 0.2 } }}
                      />
                    )}
                  </AnimatePresence>
                  <span className="relative z-10">{(isAr && d.title_ar) || d.title}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </LayoutGroup>

      {links.length > 0 && (
        // The departments sit at the top of the column and these at its foot, so the column is as
        // tall as the work area beside it without spreading the names apart.
        <nav className="hidden lg:block">
          <hr className="mb-4 border-foreground/10" />
          <ul className="flex flex-col gap-1">
            {links.map((l) => (
              <li key={l.href}>
                <Link
                  href={l.href}
                  className="block w-fit rounded-xl px-4 py-1.5 text-base font-medium text-muted-foreground outline-none transition-colors hover:bg-background hover:text-foreground focus-visible:bg-background focus-visible:text-foreground lg:-ms-4"
                >
                  {l.label}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
      )}
    </section>
  );
}
