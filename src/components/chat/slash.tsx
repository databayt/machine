"use client";

import { Fragment } from "react";

import { cn } from "@/lib/utils";
import { copy, tr } from "./config";
import type { SlashEntry } from "./keywords";
import { iconFor } from "./suggestions";

const GROUP = { 0: copy.slashFormulas, 1: copy.slashPack, 2: copy.slashSkills } as const;

/** The id the textarea names in `aria-activedescendant`. */
export const slashOptionId = (listId: string, i: number) => `${listId}-${i}`;

/**
 * The "/" menu above the input: this department's formulas, its pack's skills, then the other skills,
 * already filtered. It never takes focus (the textarea keeps it and drives the arrows); a click picks.
 */
export function SlashMenu({
  entries,
  index,
  isAr,
  listId,
  onPick,
  onHover,
}: {
  entries: SlashEntry[];
  index: number;
  isAr: boolean;
  listId: string;
  onPick: (e: SlashEntry) => void;
  onHover: (i: number) => void;
}) {
  const t = tr(isAr);
  return (
    <div
      id={listId}
      role="listbox"
      aria-label={t(copy.slashLabel)}
      dir={isAr ? "rtl" : "ltr"}
      className="absolute inset-x-2 bottom-full z-20 mb-2 max-h-72 overflow-y-auto rounded-2xl border border-border/60 bg-background p-1.5 shadow-[0_8px_30px_-12px_rgb(0_0_0/0.25)]"
    >
      {entries.map((e, i) => {
        const Icon = e.kind === "formula" ? iconFor({ icon: e.icon ?? null, label: e.title, labelEn: e.title, prompt: null }) : null;
        return (
          <Fragment key={`${e.kind}:${e.id}`}>
            {(i === 0 || entries[i - 1]!.group !== e.group) && (
              <div className="px-3 pb-1 pt-2 text-xs font-medium text-muted-foreground">{t(GROUP[e.group])}</div>
            )}
            <div
              id={slashOptionId(listId, i)}
              role="option"
              aria-selected={i === index}
              // mousedown, not click: the textarea must keep focus and its caret.
              onMouseDown={(ev) => {
                ev.preventDefault();
                onPick(e);
              }}
              onMouseEnter={() => onHover(i)}
              className={cn(
                "flex cursor-pointer items-center gap-2.5 rounded-xl px-3 py-2 text-start text-[15px]",
                i === index && "bg-muted",
              )}
            >
              {Icon && <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden />}
              <span className="min-w-0 flex-1">
                <span className="block truncate" dir="auto">
                  {e.kind === "skill" ? `/${e.name}` : e.title}
                </span>
                {(e.kind === "formula" ? e.hint : e.title !== e.name ? e.title : "") && (
                  <span className="block truncate text-xs text-muted-foreground" dir="auto">
                    {e.kind === "formula" ? e.hint : e.title}
                  </span>
                )}
              </span>
            </div>
          </Fragment>
        );
      })}
    </div>
  );
}
