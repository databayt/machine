// activity-pure — what a turn's tool events fold into. No React, so it tests alone.

/** Rotating word: each turn starts on its own, changing with the tick. */
export function playfulWord(list: readonly string[], seed: string, tick: number): string {
  if (list.length === 0) return ""
  let hash = 0
  for (const ch of seed) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0
  return list[(hash + Math.max(0, tick)) % list.length] ?? ""
}

/**
 * The turn's verb, as the CLI keeps it: picked once from the seed and held for the whole turn (the CLI does not
 * rotate it while it works).
 */
export function turnWord(list: readonly string[], seed: string): string {
  return playfulWord(list, seed, 0)
}

/** The finished steps' tool names, each once in first-seen order. */
export function uniqueTools(names: readonly string[]): string[] {
  return [...new Set(names)]
}

/** "Read, Bash, Edit +2" — up to `max` names, the rest counted. */
export function summarize(names: readonly string[], max = 3): { head: string[]; rest: number } {
  const u = uniqueTools(names)
  return { head: u.slice(0, max), rest: Math.max(0, u.length - max) }
}

/** Claude Code's spinner glyphs, in the order they grow. */
export const GLYPHS = ["·", "✢", "✳", "✶", "✻", "✽"] as const

/** One glyph frame lasts this long: the CLI steps its spinner about five times a second. */
export const GLYPH_FRAME_MS = 320

/** One character of the shimmer lights this much after the one before it, and the whole sweep repeats every `SHIMMER_CYCLE_MS`. */
export const SHIMMER_STEP_MS = 70
export const SHIMMER_CYCLE_MS = 3000

/** The glyph of frame `n`: up the list, then back down, forever (`· ✢ ✳ ✶ ✻ ✽ ✻ ✶ ✳ ✢ ·`). */
export function glyphFrame(n: number): string {
  const last = GLYPHS.length - 1
  const i = ((Math.floor(n) % (2 * last)) + 2 * last) % (2 * last)
  return GLYPHS[i <= last ? i : 2 * last - i] as string
}

const AR_DIGITS = "٠١٢٣٤٥٦٧٨٩"
const arabic = (s: string) => s.replace(/\d/g, (d) => AR_DIGITS[Number(d)] as string).replace(".", "٫")

/** Output tokens as Claude Code writes them: 950, 1.4k, 12k. Arabic: ٩٥٠ رمز · ١٫٤ ألف رمز. */
export function formatTokens(n: number, isAr: boolean): string {
  const v = Math.max(0, Math.floor(n))
  if (v < 1000) return isAr ? `${arabic(String(v))} رمز` : `${v} tokens`
  const k = Math.round(v / 100) / 10
  const text = Number.isInteger(k) ? String(k) : k.toFixed(1)
  return isAr ? `${arabic(text)} ألف رمز` : `${text}k tokens`
}

/** Elapsed time: 12s, 1m 5s. Arabic: ١٢ث, ١د ٥ث. */
export function formatElapsed(seconds: number, isAr: boolean): string {
  const s = Math.max(0, Math.floor(seconds))
  const m = Math.floor(s / 60)
  const r = s % 60
  if (isAr) return m > 0 ? `${arabic(String(m))}د ${arabic(String(r))}ث` : `${arabic(String(s))}ث`
  return m > 0 ? `${m}m ${r}s` : `${s}s`
}

/**
 * The bracket of the status line: `12s · ↓ 1.4k tokens · esc to interrupt`. The token part is left out
 * when the run has not reported any (`tokens` null or 0). `hint` is the esc text, isolated so a Latin
 * "esc" stays put inside Arabic.
 */
export function statusDetail(seconds: number, tokens: number | null, isAr: boolean, hint: string): string {
  const parts = [formatElapsed(seconds, isAr)]
  if (tokens && tokens > 0) parts.push(`↓ ${formatTokens(tokens, isAr)}`)
  parts.push(isAr ? hint.replace("esc", "⁨esc⁩") : hint)
  return parts.join(" · ")
}

/** A leading `Note: …` paragraph (the fallback notice) split from the answer: `{ note, rest }`. */
export function splitNote(text: string): { note: string | null; rest: string } {
  if (!text.startsWith("Note: ")) return { note: null, rest: text }
  const i = text.indexOf("\n\n")
  const note = (i < 0 ? text : text.slice(0, i)).slice(6).trim()
  return { note: note || null, rest: i < 0 ? "" : text.slice(i + 2).trimStart() }
}
