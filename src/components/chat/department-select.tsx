"use client";

import { useState } from "react";
import {
  BriefcaseIcon,
  CheckIcon,
  ChevronDownIcon,
  ClapperboardIcon,
  CodeXmlIcon,
  FlaskConicalIcon,
  HandCoinsIcon,
  ImageIcon,
  LayoutGridIcon,
  LifeBuoyIcon,
  MegaphoneIcon,
  type LucideIcon,
} from "lucide-react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { copy as menuCopy } from "../menu/config";
import { tr, type Copy } from "./config";

/** Each department's mark; one not listed falls back to the briefcase. */
const DEPT_ICONS: Record<string, LucideIcon> = {
  tech: CodeXmlIcon,
  business: BriefcaseIcon,
  sales: HandCoinsIcon,
  marketing: MegaphoneIcon,
  video: ClapperboardIcon,
  image: ImageIcon,
  support: LifeBuoyIcon,
  rnd: FlaskConicalIcon,
};
const deptIcon = (name: string): LucideIcon => DEPT_ICONS[name] ?? BriefcaseIcon;

/** A trigger in the row under the composer: icon, label, chevron; tinted while its menu is open. */
export const trigger =
  "inline-flex min-w-0 items-center gap-1.5 rounded-lg px-2 py-1 transition-colors hover:bg-foreground/5 hover:text-foreground disabled:opacity-50 data-[state=open]:bg-foreground/5 data-[state=open]:text-foreground";
/** A row inside a menu. */
export const row =
  "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-start text-sm text-foreground transition-colors hover:bg-muted";

export interface DepartmentOption {
  name: string;
  title: Copy;
}

/**
 * The `▣ Department ▾` select: the home screen's (under the chat input) and every page filter's.
 * `all` adds an "All" row first (the value `"all"`); without it the value must be one of `options`.
 */
export function DepartmentSelect({
  isAr,
  options,
  value,
  onChange,
  all,
  disabled,
}: {
  isAr: boolean;
  options: readonly DepartmentOption[];
  value: string;
  onChange: (name: string) => void;
  /** The "All" row's label; absent: no such row. */
  all?: Copy;
  disabled?: boolean;
}) {
  const t = tr(isAr);
  const [open, setOpen] = useState(false);
  const rows: DepartmentOption[] = all ? [{ name: "all", title: all }, ...options] : [...options];
  const current = rows.find((d) => d.name === value);
  if (!current) return null;
  const CurrentIcon = current.name === "all" ? LayoutGridIcon : deptIcon(current.name);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" disabled={disabled} aria-label={t(menuCopy.department)} className={trigger}>
          <CurrentIcon className="size-4 shrink-0" aria-hidden />
          <span className="truncate">{t(current.title)}</span>
          <ChevronDownIcon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent side="bottom" align="start" dir={isAr ? "rtl" : "ltr"} className="w-64 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5">
        <ul>
          {rows.map((d) => {
            const Icon = d.name === "all" ? LayoutGridIcon : deptIcon(d.name);
            return (
              <li key={d.name}>
                <button
                  type="button"
                  onClick={() => {
                    setOpen(false);
                    if (d.name !== value) onChange(d.name);
                  }}
                  className={cn(row)}
                >
                  <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
                  <span className="flex-1 truncate">{t(d.title)}</span>
                  {d.name === value && <CheckIcon className="size-4 shrink-0" aria-hidden />}
                </button>
              </li>
            );
          })}
        </ul>
      </PopoverContent>
    </Popover>
  );
}
