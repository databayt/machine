// chat — accepting a plan. A plan turn gets one button, Accept; it sends one fixed English message on
// the working route. A typed reply stays on the plan route (it answers or changes the plan).
// Pure: node --test src/components/chat/accept.test.ts

/** When the filled formula records to an issue: create the film issue and its first work orders. */
export const ACCEPT_ISSUE =
  "The plan is accepted. Create the film issue and its first work orders now: `produce issue <ID> --create`, then `produce order` for each piece that can start, and reply with the links. Then stop."

/** Otherwise: carry out the first step. */
export const ACCEPT_STEP = "The plan is accepted. Carry out the first step, then stop at the next gate."

/** The message Accept sends, by the `record` of the formula that opened the plan. */
export const acceptMessage = (record: string | null | undefined): string =>
  record === "issue" ? ACCEPT_ISSUE : ACCEPT_STEP

/** The route a turn runs on: the one asked for, else the one chosen on screen. */
export const turnRoute = (asked: string | null | undefined, chosen: string | null | undefined): string | null =>
  asked ?? chosen ?? null

/** The first route that is not plan mode: where accepted work runs. */
export function workingRoute<R extends { id: string; mode: string }>(routes: readonly R[]): R | null {
  return routes.find((r) => r.mode !== "plan") ?? null
}

/** A turn that ran in plan mode ends with Accept, while it is the last one and finished cleanly. */
export function showAccept(opts: {
  ranInPlan: boolean
  last: boolean
  busy: boolean
  streaming: boolean
  hasText: boolean
  failed: boolean
  hasWorkingRoute: boolean
}): boolean {
  return opts.ranInPlan && opts.last && !opts.busy && !opts.streaming && opts.hasText && !opts.failed && opts.hasWorkingRoute
}
