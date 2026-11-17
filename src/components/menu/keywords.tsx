"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";

import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import type { CatalogItem } from "../catalog/types";
import { rankKeywords } from "./keywords-rank";
import { promptHref } from "./config";
import { loadCatalog } from "./data";

/** Set size by rank: the top fifth largest, then three smaller steps. */
const size = (rank: number, n: number) => {
  const r = rank / Math.max(1, n);
  return r < 0.2 ? "text-xl font-semibold" : r < 0.45 ? "text-lg font-medium" : r < 0.75 ? "text-base" : "text-sm";
};

/** The tag cloud under the department's cards. Each tag fills the composer with the keyword. */
export function Keywords({ lang, department }: { lang: Locale; department: string }): React.JSX.Element | null {
  const [catalog, setCatalog] = useState<CatalogItem[] | null>(null);
  useEffect(() => {
    let live = true;
    void loadCatalog().then((c) => live && setCatalog(c));
    return () => {
      live = false;
    };
  }, []);
  const tags = useMemo(() => rankKeywords(catalog ?? [], department), [catalog, department]);
  if (tags.length === 0) return null;

  return (
    <ul aria-label={lang === "ar" ? "الكلمات المفتاحية" : "Keywords"} className="flex flex-wrap items-center gap-2">
      {tags.map((tag, i) => (
        <li key={tag.word}>
          <Link
            href={promptHref(lang, tag.word)}
            dir="auto"
            className={cn(
              "inline-flex items-center rounded-full bg-background px-4 py-1.5 text-foreground/80 outline-none transition-colors hover:bg-foreground hover:text-background focus-visible:bg-foreground focus-visible:text-background",
              size(i, tags.length),
            )}
          >
            {tag.word}
          </Link>
        </li>
      ))}
    </ul>
  );
}
