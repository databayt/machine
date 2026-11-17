// keywords-rank — the department's most relevant keywords for the menu's tag cloud. Pure.
import type { CatalogItem } from "../catalog/types"

/** How many tags the cloud shows at most. */
const MAX_TAGS = 40

/**
 * The department's most relevant keywords, ranked: a keyword scores once for every catalog item of
 * the department's pack that carries it (the pack itself, its skills and agents), so the words that
 * run through the department's whole kit come first and are set larger. Pure, so it is tested.
 */
export function rankKeywords(catalog: CatalogItem[], department: string): { word: string; weight: number }[] {
  const pack = catalog.find((i) => i.id === `pack:${department}`)
  if (!pack) return []
  const ids = new Set([pack.id, ...pack.dependencies])
  const score = new Map<string, number>()
  for (const item of catalog)
    if (ids.has(item.id))
      for (const k of item.keywords) {
        const word = k.trim()
        if (word) score.set(word, (score.get(word) ?? 0) + 1)
      }
  return [...score.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, MAX_TAGS)
    .map(([word, weight]) => ({ word, weight }))
}

