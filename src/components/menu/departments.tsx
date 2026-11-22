"use client";

import { CornerDownRight } from "lucide-react";
import Link from "next/link";

import { cn } from "@/lib/utils";
import type { MachineProfile } from "../profile/types";

/**
 * The menu's text items. The highlight is thmanyah's hero mark (`thmanyah/src/app/globals.css
 * .hero-mark`): a flat highlighter band, no radius, behind the middle of the line (inset ~21% from
 * the top, ~20% from the bottom of the line box). White on the menu's gray. One mark only, under the
 * SELECTED item, and no motion: it simply moves on a click; hover and focus only darken the text.
 */
export const item =
  "relative block whitespace-nowrap py-1 text-start text-lg font-semibold leading-tight tracking-tight outline-none transition-colors focus-visible:underline focus-visible:underline-offset-4 lg:text-xl";

/** An item's label, with the white mark behind it when it is the current one. */
function Label({ on, children }: { on: boolean; children: React.ReactNode }) {
  return (
    <span className="relative inline-block">
      {on && <span aria-hidden className="absolute -inset-x-px bottom-[20%] top-[21%] bg-background" />}
      <span className="relative">{children}</span>
    </span>
  );
}

/**
 * The departments this machine may use, as the menu's text items. Lives in Settings → General now;
 * the sidebar and the menu carry no departments.
 */
export function DepartmentList({
  isAr,
  profile,
  selected,
  label,
  onSelect,
}: {
  isAr: boolean;
  profile: Pick<MachineProfile, "departments">;
  selected: string;
  label: string;
  onSelect: (name: string) => void;
}): React.JSX.Element {
  return (
    <ul role="listbox" aria-label={label} className="flex flex-wrap gap-x-6 gap-y-1">
      {profile.departments.map((d) => {
        const on = d.name === selected;
        return (
          <li key={d.name} role="presentation">
            <button
              type="button"
              role="option"
              aria-selected={on}
              onClick={() => onSelect(d.name)}
              className={cn(item, on ? "text-foreground" : "text-muted-foreground hover:text-foreground")}
            >
              <Label on={on}>{(isAr && d.title_ar) || d.title}</Label>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

export interface NavLink {
  href: string;
  label: string;
  /** Dark text: this link is on the path. */
  active?: boolean;
  /** The white mark: the deepest listed link of the path. */
  mark?: boolean;
  /** The deepest link of the path (`aria-current="page"`). */
  current?: boolean;
  /** A muted count after the label. */
  hint?: string;
  /** A small dot: something here needs attention. */
  alert?: string;
  /** The dot's background class (default: destructive). */
  alertClass?: string;
  /** Literal text (a folder name) reads left to right in any language. */
  ltr?: boolean;
  /** The path under this link, at the same size, each line led by a ↳. */
  children?: NavLink[];
}

/** A column of the sidebar's links; a link with `children` lists them under it (the path doubles as the breadcrumb). */
export function NavLinks({ links, depth = 0 }: { links: NavLink[]; depth?: number }): React.JSX.Element {
  return (
    <ul className={cn("flex flex-col gap-1", depth > 0 && "mt-1", depth === 1 && "ps-1", depth > 1 && "ps-5")}>
      {links.map((l) => {
        // Only the deepest link of the path is strong; the links above it (the parent too) go quiet.
        const tone = l.current || l.mark ? "text-foreground" : "text-muted-foreground hover:text-foreground";
        return (
          <li key={l.href}>
            {/^https?:\/\//.test(l.href) ? (
              <a href={l.href} target="_blank" rel="noreferrer" className={cn(item, "text-muted-foreground hover:text-foreground")}>
                {l.label}
              </a>
            ) : (
              <Link
                href={l.href}
                aria-current={l.current ? "page" : undefined}
                className={cn(item, depth > 0 && "flex items-start gap-1.5 whitespace-normal", tone)}
              >
                {depth > 0 && <CornerDownRight aria-hidden className="mt-1.5 size-3.5 shrink-0 text-foreground/30 rtl:-scale-x-100" />}
                <span>
                  <Label on={!!l.mark}>{l.ltr ? <bdi dir="ltr">{l.label}</bdi> : l.label}</Label>
                  {l.alert && (
                    <span role="img" aria-label={l.alert} title={l.alert} className={cn("ms-1.5 inline-block size-1.5 rounded-full align-middle", l.alertClass ?? "bg-destructive")} />
                  )}
                  {l.hint && (
                    <span className="ms-1.5 text-xs font-normal text-muted-foreground/70 tabular-nums">
                      <bdi dir="ltr">{l.hint}</bdi>
                    </span>
                  )}
                </span>
              </Link>
            )}
            {l.children && l.children.length > 0 && <NavLinks links={l.children} depth={depth + 1} />}
          </li>
        );
      })}
    </ul>
  );
}
