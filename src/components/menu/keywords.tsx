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

/** Size and opacity by rank: the strongest words are larger and darker, the tail smaller and fainter. */
const weight = (rank: number, n: number) => {
  const r = rank / Math.max(1, n);
  return r < 0.15
    ? "text-2xl font-semibold text-foreground"
    : r < 0.35
      ? "text-xl font-medium text-foreground/80"
      : r < 0.6
        ? "text-lg text-foreground/60"
        : r < 0.8
          ? "text-base text-foreground/45"
          : "text-sm text-foreground/35";
};

/** Plain words, no chips. Every word sits on the same line height, so the cloud clips cleanly at two lines. */
const tagBase =
  "inline-flex h-10 items-center rounded-sm outline-none transition-colors hover:text-foreground focus-visible:underline focus-visible:underline-offset-4";

/**
 * The tag cloud under the department's cards, two rows exactly (the rest are cut, not wrapped):
 * plain words, no chips — first the repositories the department works in (bold, full ink), then its
 * most relevant keywords (ranked from its pack's catalog items: larger and darker the stronger),
 * then those repositories' feature blocks, small and faint. A tap fills the composer.
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
      className="flex max-h-[5.25rem] flex-wrap items-center gap-x-5 gap-y-1 overflow-hidden"
    >
      {repos.map((r) => (
        <li key={`repo:${r.name}`}>
          <Link
            href={promptHref(lang, r.name)}
            className={cn(tagBase, "text-2xl font-bold tracking-tight text-foreground")}
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
            className={cn(tagBase, weight(i, words.length))}
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
            className={cn(tagBase, "text-sm text-foreground/35")}
          >
            {b.block}
          </Link>
        </li>
      ))}
    </ul>
  );
}
