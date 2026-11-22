// chat — keyword matching for the composer: skills named in the text, and the slash menu's filter.
// Pure: no React, no IPC (node --test src/components/chat/keywords.test.ts).
import type { CatalogItem } from "../catalog/types.ts"
import type { Match } from "../pack/types.ts"

/** The text must have at least this many words before the station is asked about it. */
export const PROBE_MIN_WORDS = 3
/** The lowest score at which a skill is offered or underlined: a phrase, or a word of the department's own kit. */
export const SKILL_MIN_SCORE = 2

/** The fields of a catalog item the matcher reads. */
export type SkillItem = Pick<CatalogItem, "id" | "name" | "title" | "description" | "keywords" | "dependencies"> &
  Partial<Pick<CatalogItem, "title_ar" | "description_ar">>

export interface SkillMatch {
  id: string
  name: string
  score: number
  /** `[start, end)` offsets into the original text. */
  spans: [number, number][]
}

const MARKS = /[ً-ٰٟـؐ-ؚ]/g
const ALEF = /[أإآٱ]/g

/** One word, folded: lowercase, no diacritics or tatweel, alef forms joined, ة→ه, ى→ي, a leading "ال" dropped. */
export function normWord(w: string): string {
  let s = w.normalize("NFC").toLowerCase().replace(MARKS, "").replace(ALEF, "ا").replace(/ة/g, "ه").replace(/ى/g, "ي")
  if (s.length > 4 && s.startsWith("ال")) s = s.slice(2)
  return s
}

interface Token {
  norm: string
  start: number
  end: number
}

/** Words (letters, numbers, marks) with their offsets in the original text. */
export function tokenize(text: string): Token[] {
  const out: Token[] = []
  for (const m of text.matchAll(/[\p{L}\p{N}\p{M}]+/gu)) {
    const norm = normWord(m[0])
    if (norm) out.push({ norm, start: m.index, end: m.index + m[0].length })
  }
  return out
}

const termWords = (term: string) => tokenize(term).map((t) => t.norm)

/** The words a description lists after "Triggers on:" (the skills' frontmatter convention). */
function triggersOf(description: string): string[] {
  const m = description.match(/triggers on:?\s*(.+)$/i)
  return m ? m[1]!.split(/[,،]/).map((x) => x.trim()).filter(Boolean) : []
}

const isSkill = (i: SkillItem) => i.id.startsWith("skill:")

/** The catalog ids of the department's own kit: its pack's dependencies. */
export function packIds(items: SkillItem[], dept: string): Set<string> {
  const pack = items.find((i) => i.id === `pack:${dept}`)
  return new Set(pack?.dependencies ?? [])
}

/** True when the machine hides the item (by catalog id or bare name). */
const isHidden = (i: SkillItem, hidden: readonly string[]) => hidden.includes(i.id) || hidden.includes(i.name)

/**
 * Skills the text names, best first. A multi-word phrase outscores a single word, the department's pack
 * items outscore the rest, and a hidden skill never appears. Arabic is folded (alef, taa marbuta,
 * diacritics, the article), so «الفيديو» meets «فيديو». Spans point into the original text.
 */
export function matchSkills(text: string, items: SkillItem[], dept: string, hidden: readonly string[] = []): SkillMatch[] {
  const toks = tokenize(text)
  if (!toks.length) return []
  const kit = packIds(items, dept)
  const out: SkillMatch[] = []
  for (const item of items) {
    if (!isSkill(item) || isHidden(item, hidden)) continue
    const terms = [...item.keywords, ...triggersOf(item.description), ...(item.title ? [item.title] : []), item.name]
    let best = 0
    const spans: [number, number][] = []
    for (const term of terms) {
      const words = termWords(term)
      if (!words.length) continue
      for (let i = 0; i + words.length <= toks.length; i++) {
        if (!words.every((w, k) => toks[i + k]!.norm === w)) continue
        // A phrase of two words or more is worth 3; a single word 1; the department's own kit adds 2.
        const base = words.length > 1 ? 3 : 1
        best = Math.max(best, base + (kit.has(item.id) ? 2 : 0) + (term === item.name ? 0.5 : 0))
        spans.push([toks[i]!.start, toks[i + words.length - 1]!.end])
      }
    }
    if (best >= SKILL_MIN_SCORE) out.push({ id: item.id, name: item.name, score: best, spans: mergeSpans(spans) })
  }
  return out.sort((a, b) => b.score - a.score || a.name.localeCompare(b.name))
}

/** Sorted, with overlapping or touching spans joined. */
export function mergeSpans(spans: [number, number][]): [number, number][] {
  const s = [...spans].sort((a, b) => a[0] - b[0] || a[1] - b[1])
  const out: [number, number][] = []
  for (const [a, b] of s) {
    const last = out.at(-1)
    if (last && a <= last[1]) last[1] = Math.max(last[1], b)
    else out.push([a, b])
  }
  return out
}

/** Every span of every match, merged: what the composer underlines. */
export const allSpans = (m: SkillMatch[]): [number, number][] => mergeSpans(m.flatMap((x) => x.spans))

/** True when the draft is worth asking the station about: three words or more, and not a slash command. */
export function shouldProbe(text: string): boolean {
  const t = text.trim()
  if (!t || t.startsWith("/") || t.length > 600) return false
  return t.split(/\s+/).length >= PROBE_MIN_WORDS
}

/** The "/fragment" under the caret: it starts the text or follows whitespace and holds no whitespace or "/". */
export function slashContext(text: string, caret: number): { start: number; fragment: string } | null {
  const before = text.slice(0, caret)
  const m = before.match(/(?:^|\s)\/([^\s/]*)$/)
  if (!m) return null
  return { start: before.length - m[1]!.length - 1, fragment: m[1]! }
}

/** One row of the slash menu. */
export interface SlashEntry {
  kind: "formula" | "skill"
  /** A formula id, or a catalog id. */
  id: string
  /** What is typed after "/": a skill's name, a formula's id. */
  name: string
  title: string
  hint: string
  keywords: string[]
  /** 0 the department's formulas, 1 its pack's skills, 2 the other skills. */
  group: 0 | 1 | 2
  icon?: string | null
}

/**
 * The entries a "/fragment" keeps: a name prefix first, then a substring of the name, title or
 * keywords; inside each tier the groups keep their order (formulas, the pack's skills, the rest).
 * An empty fragment keeps all, in group order.
 */
export function slashFilter(fragment: string, entries: SlashEntry[]): SlashEntry[] {
  const q = normWord(fragment.trim())
  const tier = (e: SlashEntry): number => {
    if (!q) return 0
    const name = normWord(e.name)
    if (name.startsWith(q)) return 0
    if ([e.name, e.title, e.hint, ...e.keywords].some((s) => s.toLowerCase().normalize("NFC").includes(fragment.trim().toLowerCase()) || normWord(s).includes(q))) return 1
    return -1
  }
  return entries
    .map((e, i) => ({ e, i, t: tier(e) }))
    .filter((x) => x.t >= 0)
    .sort((a, b) => a.t - b.t || a.e.group - b.e.group || a.i - b.i)
    .map((x) => x.e)
}

/** The ghost chip above the box: a formula the station matched, or a skill the text names. */
export type Chip =
  | { kind: "formula"; forText: string; id: string; label: string; facts: string[]; match: Match & { id: string } }
  | { kind: "skill"; forText: string; name: string }

/** Up to three slot values the sentence itself gave, short enough for one quiet line. */
export function chipFacts(set: Record<string, string> | null | undefined): string[] {
  return Object.values(set ?? {})
    .map((v) => String(v ?? "").trim())
    .filter(Boolean)
    .slice(0, 3)
    .map((v) => (v.length > 28 ? `${v.slice(0, 27)}…` : v))
}

/** Where the first open slot sits in a filled prompt (the script writes `[unfilled: name]`), else null. */
export function firstOpenSlot(prompt: string): [number, number] | null {
  const m = /\[unfilled:[^\]]*\]/.exec(prompt)
  return m ? [m.index, m.index + m[0].length] : null
}
