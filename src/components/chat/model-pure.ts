// chat — the model the next run uses, as the chip shows it. Pure (node --test src/components/chat/model.test.ts).

/** The Claude models a person may pick for one run. Fable is never offered. */
export const FAMILIES = ["opus", "sonnet", "haiku"] as const
export type Family = (typeof FAMILIES)[number]

const NAMES: Record<Family, string> = { opus: "Opus 5.5", sonnet: "Sonnet 5.5", haiku: "Haiku 5.5" }

/** The family a model string names (`opus`, `claude-sonnet-5-5`), or null for anything else, Fable included. */
export function familyOf(model: string | null | undefined): Family | null {
  const m = (model ?? "").toLowerCase()
  if (m.includes("fable")) return null
  return FAMILIES.find((f) => m.includes(f)) ?? null
}

/** "Opus 5.5"; "Claude" when the route names no model. */
export function modelName(model: string | null | undefined): string {
  const f = familyOf(model)
  return f ? NAMES[f] : "Claude"
}

/** The model the next run uses: the person's one-run pick, else the formula's, else the route's. */
export function effectiveModel(override: Family | null, formula: string | null, route: string | null): string | null {
  return override ?? (familyOf(formula) ? formula : null) ?? route
}
