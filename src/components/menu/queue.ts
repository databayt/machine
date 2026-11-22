// menu — the queue of upcoming issues: pure ordering (node --test src/components/menu/queue.test.ts).
import type { Task } from "../tasks/types.ts"

export const MAX_CARDS = 6

interface View {
  mine: Task[]
  board: Task[]
}

/**
 * The upcoming issues: the ones I hold, then the orders assigned to me (even outside my departments),
 * then the free board in queue order, others' last. Each issue once.
 */
export function upcoming(view: View | null, login: string | undefined): Task[] {
  if (!view) return []
  const seen = new Set<string>()
  const out: Task[] = []
  const assigned = (t: Task) => !!login && t.assignee === login
  const free = (t: Task) => !t.holder || t.holder === login
  const all = [...view.mine, ...view.board]
  for (const t of [...view.mine, ...all.filter(assigned), ...view.board.filter(free), ...view.board.filter((x) => !free(x))]) {
    if (seen.has(t.id)) continue
    seen.add(t.id)
    out.push(t)
  }
  return out.slice(0, MAX_CARDS)
}
