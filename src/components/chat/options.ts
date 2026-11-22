// chat — the home options: the station's board first, the pack's quick actions as the fallback.
// Pure: no React, no IPC (node --test src/components/chat/options.test.ts).
import type { Board, BoardOption, QuickAction } from "../pack/types.ts"

/** The most options the home shows. */
export const MAX_OPTIONS = 3
/** The lowest switch confidence at which typed text is filled by the station instead of refined by a model. */
export const MATCH_MIN = 0.6

/** One row of the home list, whichever source it came from. */
export interface HomeOption {
  /** Unique within the list; the formula id for a station option. */
  key: string
  /** The formula id the tap fills (station), or null (a quick action). */
  formula: string | null
  /** The id the station log records: the formula id, or a quick action's own id; null when it has none. */
  logId: string | null
  label: string
  /** A lucide icon name from the formula; null for the keyword heuristics. */
  icon: string | null
  /** The slot values handed back to `fill`. */
  set: Record<string, string>
  /** A quick action's brief, written into the box as it is. */
  prompt: string | null
  /** The route to run it on, when the source names one. */
  route: string | null
  /** Quick actions still go through refine after a tap; station options never do. */
  refine: boolean
  /** The English label, the text a failed fill falls back to. */
  labelEn: string
}

const clean = (s: string | null | undefined) => (s ?? "").trim()

/** The board's options as home rows, in the script's order, at most three, ids unique. */
export function optionsFromBoard(board: Board | null, isAr: boolean): HomeOption[] {
  const seen = new Set<string>()
  const out: HomeOption[] = []
  for (const o of board?.options ?? []) {
    if (!o.id || seen.has(o.id)) continue
    seen.add(o.id)
    const label = clean(isAr ? o.label_ar || o.label_en : o.label_en || o.label_ar)
    out.push({
      key: o.id,
      formula: o.id,
      logId: o.id,
      label: label || o.id,
      icon: o.icon,
      set: { ...o.set },
      prompt: null,
      route: o.route,
      refine: false,
      labelEn: clean(o.label_en) || o.id,
    })
  }
  return out.slice(0, MAX_OPTIONS)
}

/** Today's quick-action logic: the repo's own actions, else the ones for any repo, first three. */
export function optionsFromQuick(all: QuickAction[], repo: string, isAr: boolean): HomeOption[] {
  const scoped = repo ? all.filter((q) => q.repos.includes(repo)) : []
  const shown = scoped.length ? scoped : all.filter((q) => q.repos.length === 0)
  return shown.slice(0, MAX_OPTIONS).map((q, i) => ({
    key: `${q.id ?? "quick"}:${i}:${q.prompt}`,
    formula: null,
    logId: q.id,
    label: clean(isAr ? q.label_ar || q.label_en : q.label_en || q.label_ar),
    icon: null,
    set: {},
    prompt: q.prompt,
    route: q.route,
    refine: true,
    labelEn: clean(q.label_en) || q.prompt,
  }))
}

/** The board when it has options, else the quick actions: other departments keep working. */
export function homeOptions(board: Board | null, quick: QuickAction[], repo: string, isAr: boolean): HomeOption[] {
  const fromBoard = optionsFromBoard(board, isAr)
  return fromBoard.length ? fromBoard : optionsFromQuick(quick, repo, isAr)
}

/** `"Play"`, `"PlayIcon"`, `"play-circle"` → `"play"`, `"playcircle"`: one key per lucide icon. */
export function iconKey(name: string | null | undefined): string {
  return (name ?? "").replace(/icon$/i, "").replace(/[^a-z0-9]/gi, "").toLowerCase()
}

/** A station match is used only with an id and enough confidence. */
export function usableMatch(m: { id: string | null; confidence: number } | null): m is { id: string; confidence: number } {
  return !!m && !!m.id && Number.isFinite(m.confidence) && m.confidence >= MATCH_MIN
}

/** A board option's slots as the log and `fill` take them. */
export const slotsOf = (o: Pick<BoardOption, "set">): Record<string, string> => ({ ...o.set })
