// chat — the repository menu's pure side (node --test src/components/chat/repos.test.ts).
import type { RepoEntry } from "./types.ts"

/** What the menu shows for a repo: its word when the registry has one, else its name. */
export const repoLabel = (e: RepoEntry): string => e.word || e.name

/** The saved repo when it is still on the menu, else the first entry (hogwarts leads), else none. */
export function pickRepo(saved: string | undefined, entries: readonly RepoEntry[]): string {
  if (saved && entries.some((e) => e.name === saved)) return saved
  return entries[0]?.name ?? ""
}

/** The entries other than `current` that match a search (by name or word, case-insensitive). */
export function filterRepos(entries: readonly RepoEntry[], current: string, query: string): RepoEntry[] {
  const q = query.trim().toLowerCase()
  return entries.filter(
    (e) => e.name !== current && (!q || e.name.toLowerCase().includes(q) || (e.word ?? "").toLowerCase().includes(q)),
  )
}
