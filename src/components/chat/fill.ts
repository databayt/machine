// chat — the navigator's sentence: pure rendering of a fill / formula template for a path.
import type { Formula, Starter } from "../pack/types.ts"

export const labelOf = (n: Starter, isAr: boolean) => (isAr ? n.label_ar : n.label_en) || n.label_en || n.label_ar

/** Facets of a path: every `set` key, plus `<key>_label` = the chosen node's label. */
export function facetsOf(path: Starter[], repo: string, isAr: boolean): Record<string, string> {
  const out: Record<string, string> = { repo }
  for (const n of path)
    for (const [k, v] of Object.entries(n.set)) {
      out[k] = v
      out[`${k}_label`] = labelOf(n, isAr)
    }
  return out
}

/** Fills `{{token}}`s; a token with no value stays as written (the legacy send path). */
export function fillPrompt(prompt: string, facets: Record<string, string>): string {
  return prompt.replace(/\{\{(\w+)\}\}/g, (m, k: string) => facets[k] ?? m)
}

const SEP_BEFORE = /\s*[·—،]\s*$/
const SEP_AFTER = /^\s*[·—،]\s*/

/**
 * Renders a template: a token with no value is dropped together with one adjacent separator
 * (" · ", " — ", "، ": the one before it, else the one after), so a partial selection reads
 * cleanly. Double spaces collapse, space before closing punctuation goes, the result is trimmed.
 */
export function renderTemplate(template: string, facets: Record<string, string>): string {
  const parts = template.split(/\{\{(\w+)\}\}/)
  // parts: literal, token, literal, token, …, literal
  const lit: string[] = []
  const tok: (string | null)[] = []
  for (let i = 0; i < parts.length; i += 2) {
    lit.push(parts[i]!)
    if (i + 1 < parts.length) {
      const v = facets[parts[i + 1]!]
      tok.push(v ? v : null)
    }
  }
  for (let i = 0; i < tok.length; i++) {
    if (tok[i] !== null) continue
    if (SEP_BEFORE.test(lit[i]!)) lit[i] = lit[i]!.replace(SEP_BEFORE, "")
    else lit[i + 1] = lit[i + 1]!.replace(SEP_AFTER, "")
  }
  let out = lit[0]!
  for (let i = 0; i < tok.length; i++) out += (tok[i] ?? "") + lit[i + 1]!
  return out
    .replace(/[ \t]{2,}/g, " ")
    .replace(/\s+([.,،:;!?؟])/g, "$1")
    .trim()
}

/** The sentence for a path: the deepest node's fill, else the pack's formula; "" when neither. */
export function sentenceFor(path: Starter[], repo: string, isAr: boolean, formula: Formula | null): string {
  const node = path.at(-1)
  if (!node) return ""
  const own = isAr ? node.fill_ar || node.fill_en : node.fill_en || node.fill_ar
  const base = own ?? (formula ? (isAr ? formula.ar || formula.en : formula.en || formula.ar) : "")
  return base ? renderTemplate(base, facetsOf(path, repo, isAr)) : ""
}
