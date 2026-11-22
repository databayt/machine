// chat — accepting a plan. A plan turn gets one button, Accept; it sends one fixed English message on
// the working route, in the same session (`--resume`), so the plan above is already in the run's context:
// the message says that plan is the next phase's prompt, it does not repeat it. A typed reply stays on the plan route (it answers or changes the plan).
// Pure: node --test src/components/chat/accept.test.ts

/** A film plan (`record: "film"`): the production, its film issue, and a work order per piece handed to a person. */
export const ACCEPT_FILM =
  "The plan above is accepted: it is the prompt for the next phase. Create the production if it does not exist yet (`produce init`), then its film issue: `produce issue <ID> --create`. For each piece the plan gives to another person, create its work order: `produce order <ID> <formula> --to <login> --create`. Reply with the links, then stop at the next gate."

/** A work order (`record: "issue"`): the request becomes one issue for one person. */
export const ACCEPT_ISSUE =
  "The plan above is accepted: it is the prompt for the next phase. Create the work order as an issue now and reply with its link. Then stop."

/**
 * A filled work order is a request for ANOTHER person. Sent as it is, the lead would do the work
 * itself; this preface turns the send into "create the issue". The text after it is the issue body.
 */
export const orderMessage = (text: string, formulaId: string): string =>
  `This is a work order for another person, not work for you. Create it as a GitHub issue for the person it names, reply with the link, then stop; do not do the work yourself. For a film: \`produce order <film ID> ${formulaId} --to <login> --set …\` with the values below (print the draft first, then add \`--create\`). Otherwise: an issue labelled \`dept:<department>\` and \`for:<login>\` whose body is the text below.\n\n---\n\n${text}`

/** Otherwise: carry out the first step. */
export const ACCEPT_STEP = "The plan above is accepted: it is the prompt for the next phase. Execute it now, starting with its first step, and stop at the next gate."

/** The message Accept sends, by the `record` of the formula that opened the plan. */
export const acceptMessage = (record: string | null | undefined): string =>
  record === "film" ? ACCEPT_FILM : record === "issue" ? ACCEPT_ISSUE : ACCEPT_STEP

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
  /** The turn ended with questions: the plan is not written yet. */
  asking?: boolean
}): boolean {
  return !opts.asking && opts.ranInPlan && opts.last && !opts.busy && !opts.streaming && opts.hasText && !opts.failed && opts.hasWorkingRoute
}
