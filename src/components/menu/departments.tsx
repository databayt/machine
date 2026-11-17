"use client";

import Link from "next/link";

import { cn } from "@/lib/utils";
import type { MachineProfile } from "../profile/types";

/**
 * Column 1: the departments, then a divider, the app links, another divider and the icon row; on large
 * screens the column sits at the top of the work area, level with the banner; a scroll row on phones.
 *
 * The highlight is thmanyah's hero mark (`thmanyah/src/app/globals.css .hero-mark`): a flat
 * highlighter band — no radius — behind the middle of the line (inset ~21% from the top, ~20% from
 * the bottom of the line box). White here, on the menu's gray. One mark only, under the SELECTED
 * department, and no motion: it simply moves on a click; hover and focus only darken the text.
 */
const item =
  "relative block whitespace-nowrap py-1 text-start text-lg font-semibold leading-tight tracking-tight outline-none transition-colors focus-visible:underline focus-visible:underline-offset-4 lg:text-xl";

export function Departments({
  isAr,
  profile,
  selected,
  label,
  onSelect,
  first,
  links = [],
  tools,
}: {
  isAr: boolean;
  profile: MachineProfile;
  selected: string;
  label: string;
  onSelect: (name: string) => void;
  first: React.Ref<HTMLButtonElement>;
  /** Under a divider, after the departments: Settings, Catalog, Sessions… */
  links?: { href: string; label: string }[];
  /** Under a second divider, the last row: the icon tools (replaces the footer on large screens). */
  tools?: React.ReactNode;
}): React.JSX.Element {
  return (
    <section className="min-w-0 lg:flex lg:flex-col lg:self-start">
      <ul
        role="listbox"
        aria-label={label}
        className="-mx-6 flex gap-5 overflow-x-auto px-6 pb-1 lg:mx-0 lg:flex-col lg:gap-1 lg:overflow-visible lg:px-0"
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
                className={cn(item, on ? "text-foreground" : "text-muted-foreground hover:text-foreground")}
              >
                <span className="relative inline-block">
                  {on && (
                    <span aria-hidden className="absolute -inset-x-px bottom-[20%] top-[21%] bg-background" />
                  )}
                  <span className="relative">{(isAr && d.title_ar) || d.title}</span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>

      {links.length > 0 && (
        // Right under the departments, the whole column aligned to the top of the work area; a
        // narrow block keeps the dividers short. Same type as above.
        <nav className="hidden lg:mt-8 lg:block lg:w-36">
          <hr className="mb-4 border-foreground/10" />
          <ul className="flex flex-col gap-1">
            {links.map((l) => (
              <li key={l.href}>
                {/^https?:\/\//.test(l.href) ? (
                  // An outside page (Docs on kun.databayt.org) opens outside the app window.
                  <a
                    href={l.href}
                    target="_blank"
                    rel="noreferrer"
                    className={cn(item, "text-muted-foreground hover:text-foreground")}
                  >
                    {l.label}
                  </a>
                ) : (
                  <Link href={l.href} className={cn(item, "text-muted-foreground hover:text-foreground")}>
                    {l.label}
                  </Link>
                )}
              </li>
            ))}
          </ul>
          {tools && (
            <>
              <hr className="my-4 border-foreground/10" />
              {tools}
            </>
          )}
        </nav>
      )}
    </section>
  );
}
