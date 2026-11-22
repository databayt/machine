import type { Locale } from "@/components/local/config"

import type { Entry } from "./types"

const DIR_OF_KIND: Record<string, Entry["dir"]> = {
  skill: "skills",
  agent: "agents",
  rule: "rules",
  hook: "hooks",
  pack: "departments",
}

/** The folder an id (`skill:funnel`) lives in. */
export const dirOf = (id: string): string => (id.startsWith("mcp:") ? "mcp" : (DIR_OF_KIND[id.split(":")[0]] ?? ""))

/** `/<lang>/library/?dir=<dir>&item=<id>` */
export const itemHref = (lang: Locale, e: { id: string }): string =>
  `/${lang}/library/?dir=${dirOf(e.id)}&item=${encodeURIComponent(e.id)}`
