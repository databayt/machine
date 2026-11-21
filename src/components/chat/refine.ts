/**
 * Refine's reply → the brief and the route it recommends. The model ends with `route: <id>`; that
 * line is stripped, and kept only when it names one of the department's routes. A reply wrapped in
 * a code fence is unwrapped.
 */
export function parseRefined(raw: string, routeIds: readonly string[]): { text: string; route: string | null } {
  let lines = raw.trim().split("\n")
  if (lines[0]?.startsWith("```") && lines.at(-1)?.trim() === "```") lines = lines.slice(1, -1)
  let route: string | null = null
  // The route line may sit before or after the closing fence; take the last one.
  for (let i = lines.length - 1; i >= 0; i--) {
    const m = /^\s*\**route\**\s*[:：]\s*`?([a-z0-9-]+)`?\s*$/i.exec(lines[i])
    if (m) {
      const id = m[1].toLowerCase()
      route = routeIds.includes(id) ? id : null
      lines = [...lines.slice(0, i), ...lines.slice(i + 1)]
      break
    }
    if (lines[i].trim()) break
  }
  if (lines[0]?.startsWith("```") && lines.at(-1)?.trim() === "```") lines = lines.slice(1, -1)
  return { text: lines.join("\n").trim(), route }
}
