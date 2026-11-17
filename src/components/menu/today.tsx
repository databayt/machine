import Link from "next/link";
import { SparklesIcon, ZapIcon } from "lucide-react";

import type { Locale } from "@/components/local/config";
import type { Task } from "../tasks/types";
import type { Department } from "../profile/types";
import type { CatalogItem } from "./data";
import { tr } from "../chat/config";
import { copy, promptHref, skillStarter } from "./config";

type Pill = { key: string; label: string; href: string; title?: string; kind: "action" | "skill" | "keyword" };

/**
 * What the machine thinks you will run today, ranked: skills named by the upcoming tasks, the
 * department's quick actions, its skills, then the keywords of its pack. Workflows before keywords.
 */
export function pillsFor({
  lang,
  dept,
  tasks,
  catalog,
  hidden,
  cap = 12,
}: {
  lang: Locale;
  dept: Department | undefined;
  tasks: Task[];
  catalog: CatalogItem[];
  hidden: string[];
  cap?: number;
}): Pill[] {
  const isAr = lang === "ar";
  const byId = new Map(catalog.map((c) => [c.id, c]));
  const seen = new Set<string>();
  const workflows: Pill[] = [];
  const keywords: Pill[] = [];
  const add = (list: Pill[], p: Pill) => {
    const k = p.label.trim().toLowerCase();
    if (!k || seen.has(k)) return;
    seen.add(k);
    list.push(p);
  };
  const skill = (name: string, title: string, description?: string) =>
    add(workflows, {
      key: `skill:${name}`,
      label: title,
      href: promptHref(lang, skillStarter(title, isAr)),
      title: description,
      kind: "skill",
    });

  for (const id of tasks.flatMap((t) => t.kun.items)) {
    if (!id.startsWith("skill:") || hidden.includes(id)) continue;
    const name = id.slice(6);
    const own = dept?.skills.find((s) => s.name === name);
    const c = byId.get(id);
    skill(name, own?.title ?? (isAr ? c?.title_ar : undefined) ?? c?.title ?? name, own?.description ?? c?.description);
  }
  for (const q of dept?.quick_actions ?? [])
    add(workflows, { key: `qa:${q.prompt}`, label: isAr ? q.label_ar : q.label_en, href: promptHref(lang, q.prompt), kind: "action" });
  for (const s of dept?.skills ?? []) skill(s.name, s.title, s.description);

  const pack = catalog.find((c) => c.type === "pack" && (c.name === dept?.name || c.id === `pack:${dept?.name}`));
  for (const dep of pack?.dependencies ?? []) {
    if (hidden.includes(dep)) continue;
    for (const k of byId.get(dep)?.keywords ?? [])
      add(keywords, { key: `kw:${k}`, label: k, href: promptHref(lang, k), kind: "keyword" });
  }
  return [...workflows, ...keywords].slice(0, cap);
}

/** Row 2: the pills. Tapping one fills the composer. */
export function Today({ lang, pills }: { lang: Locale; pills: Pill[] }): React.JSX.Element | null {
  const t = tr(lang === "ar");
  if (pills.length === 0) return null;
  return (
    <ul aria-label={t(copy.todayHint)} title={t(copy.todayHint)} className="flex flex-wrap gap-2.5">
      {pills.map((p) => (
        <li key={p.key}>
          <Link
            href={p.href}
            title={p.title ?? t(copy.todayHint)}
            dir="auto"
            className="inline-flex items-center gap-2 rounded-full bg-muted px-4 py-2.5 text-base font-medium transition-colors hover:bg-foreground hover:text-background"
          >
            {p.kind === "action" && <ZapIcon aria-hidden className="size-4 opacity-60" />}
            {p.kind === "skill" && <SparklesIcon aria-hidden className="size-4 opacity-60" />}
            {p.label}
          </Link>
        </li>
      ))}
    </ul>
  );
}
