// chat — when a user's message is long enough to fold. Pure: no React (node --test src/components/chat/clamp.test.ts).

/** A folded message shows this many lines. */
export const CLAMP_LINES = 3

/** About how many characters fit on one line of the user's bubble (a 48 rem column, 85% of it, 16 px text). */
export const LINE_CHARS = 64

export interface Clamp {
  /** The message takes more than `max` lines: fold it. */
  clamped: boolean
  /** Lines the message takes, counting a long line as it wraps. */
  lines: number
}

/**
 * Lines a message takes: each hard line counts once, and a line longer than `cols` characters counts again for every
 * further `cols` it wraps over. An estimate, because the real wrap depends on the width — the bubble clamps with CSS
 * (`line-clamp`) and this only decides whether to offer Show more.
 */
export function clampLines(text: string, max = CLAMP_LINES, cols = LINE_CHARS): Clamp {
  const trimmed = text.replace(/\s+$/, "")
  if (!trimmed) return { clamped: false, lines: 0 }
  let lines = 0
  for (const line of trimmed.split("\n")) lines += Math.max(1, Math.ceil(Array.from(line).length / cols))
  return { clamped: lines > max, lines }
}

/** The first message of a session starts folded; any other starts open (it is usually one line). */
export const startsFolded = (index: number): boolean => index === 0
