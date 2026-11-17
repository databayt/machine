"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";

import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import type { CatalogItem } from "../catalog/types";
import type { Department } from "../profile/types";
import { promptHref } from "./config";
import { loadCatalog } from "./data";
import { rankKeywords } from "./keywords-rank";

/** Set size by rank: the top fifth largest, then three smaller steps. */
const size = (rank: number, n: number) => {
  const r = rank / Math.max(1, n);
  return r < 0.2 ? "text-xl font-semibold" : r < 0.45 ? "text-lg font-medium" : r < 0.75 ? "text-base" : "text-sm";
};

/** Every tag is this tall whatever its type size, so the cloud clips cleanly at two rows. */
const tagBase =
  "inline-flex h-10 items-center rounded-full px-4 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring";

/**
 * The tag cloud under the department's cards, two rows exactly (the rest are cut, not wrapped):
 * first the repositories the department works in — dark tags, so they read apart from keywords —
 * then its most relevant keywords (ranked from its pack's catalog items, the strongest set larger),
 * then those repositories' feature blocks. A tap fills the composer.
 */
export function Keywords({ lang, department }: { lang: Locale; department: Department | undefined }): React.JSX.Element | null {
  const [catalog, setCatalog] = useState<CatalogItem[] | null>(null);
  useEffect(() => {
    let live = true;
    void loadCatalog().then((c) => live && setCatalog(c));
    return () => {
      live = false;
    };
  }, []);

  const name = department?.name ?? "";
  const repos = department?.repos ?? [];
  const words = useMemo(() => rankKeywords(catalog ?? [], name), [catalog, name]);
  const blocks = useMemo(() => {
    const seen = new Set(words.map((w) => w.word));
    const out: { repo: string; block: string }[] = [];
    // Interleave the repos' blocks, so one big repo doesn't fill the line alone.
    for (let i = 0; i < 40; i += 1)
      for (const r of repos) {
        const block = r.blocks[i];
        if (block && !seen.has(block)) {
          seen.add(block);
          out.push({ repo: r.name, block });
        }
      }
    return out;
  }, [repos, words]);

  if (!department || (repos.length === 0 && words.length === 0 && blocks.length === 0)) return null;

  return (
    <ul
      aria-label={lang === "ar" ? "الكلمات المفتاحية" : "Keywords"}
      className="flex max-h-[5.5rem] flex-wrap items-center gap-2 overflow-hidden"
    >
      {repos.map((r) => (
        <li key={`repo:${r.name}`}>
          <Link
            href={promptHref(lang, r.name)}
            className={cn(tagBase, "bg-foreground text-lg font-semibold text-background hover:bg-foreground/85")}
          >
            {r.name}
          </Link>
        </li>
      ))}
      {words.map((w, i) => (
        <li key={`kw:${w.word}`}>
          <Link
            href={promptHref(lang, w.word)}
            dir="auto"
            className={cn(tagBase, "bg-background text-foreground/80 hover:bg-foreground hover:text-background", size(i, words.length))}
          >
            {w.word}
          </Link>
        </li>
      ))}
      {blocks.map((b) => (
        <li key={`block:${b.repo}:${b.block}`}>
          <Link
            href={promptHref(lang, `${b.repo} ${b.block}`)}
            title={`${b.repo} · ${b.block}`}
            className={cn(tagBase, "bg-background text-sm text-foreground/70 hover:bg-foreground hover:text-background")}
          >
            {b.block}
          </Link>
        </li>
      ))}
    </ul>
  );
}
