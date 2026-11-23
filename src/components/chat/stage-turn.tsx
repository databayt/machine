"use client"

import { useState } from "react"

import type { Answered } from "./ask-pure"
import type { SpecView } from "./block-pure"
import { copy, tr } from "./config"
import { type StageView, stageHeading } from "./stage-pure"
import { Wizard } from "./wizard"

/**
 * One stage of a film, drawn in the chat with no model run: `▸ 07 visual · locked by abdout`, its summary lines and,
 * while it is the latest turn, the question wizard over the stage's standard choices, or one Discuss button when it has
 * none (or the wizard was cancelled). Nothing here starts a model by itself: the handlers do, when the person acts.
 */
export function StageTurn({
  view,
  active,
  isAr,
  title,
  onAnswer,
  onDiscuss,
}: {
  view: StageView
  /** The latest turn, nothing running: the only time the wizard or the button shows. */
  active: boolean
  isAr: boolean
  /** The wizard's bottom-rule title. */
  title: string
  /** The wizard submitted: the picks and anything typed (a pick locks the stage by script, typed text opens the discussion). */
  onAnswer: (answers: Answered[]) => void
  /** "Chat about this" or Discuss: the agent opens the stage discussion (typed text is handled by `onAnswer`). */
  onDiscuss: () => void
}) {
  const t = tr(isAr)
  const [closed, setClosed] = useState(false)
  const heading = stageHeading(view, { locked: t(copy.stageLocked), open: t(copy.stageOpen), by: t(copy.stageBy) })
  const wizard = active && view.questions.length > 0 && !closed
  return (
    <li dir="auto" className="min-w-0 font-mono text-[13px] leading-6 text-foreground/75">
      <p className="flex items-baseline gap-[1ch] text-start">
        <span aria-hidden dir="ltr" className="shrink-0 text-[color:var(--cli-dim)]">▸</span>
        <span className="min-w-0 break-words font-medium text-foreground">{heading}</span>
      </p>
      {view.summary.map((line, i) => (
        <p key={i} className="flex items-baseline gap-[1ch] text-start">
          <span aria-hidden dir="ltr" className="w-[1ch] shrink-0" />
          <span className="min-w-0 break-words">{line}</span>
        </p>
      ))}
      {wizard && (
        <Wizard
          questions={view.questions}
          isAr={isAr}
          title={title}
          startOnChat={view.ask === "agent"}
          onSubmit={onAnswer}
          onCancel={() => setClosed(true)}
          onChat={onDiscuss}
        />
      )}
      {active && !wizard && (
        <button
          type="button"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onDiscuss}
          className="mt-2 inline-flex items-center rounded-full bg-foreground px-4 py-2 font-sans text-sm font-medium text-background transition-colors hover:bg-foreground/85"
        >
          {t(copy.discuss)}
        </button>
      )}
    </li>
  )
}

/**
 * A stage discussion's result (a `kun-spec` block): a small summary — the decisions, the tool, the notes — and, while it
 * is the latest turn, a Lock button that records it in the film's ledger by script (no model).
 */
export function SpecCard({
  spec,
  isAr,
  canLock,
  error,
  onLock,
}: {
  spec: SpecView
  isAr: boolean
  /** The latest turn and nothing running. */
  canLock: boolean
  error: string | null
  onLock: () => void
}) {
  const t = tr(isAr)
  const rows = Object.entries(spec.decisions).map(([k, v]) => `${k}: ${typeof v === "string" ? v : JSON.stringify(v)}`)
  if (spec.tool) rows.push(`${t(copy.specTool)}: ${spec.tool}`)
  return (
    <div dir="auto" className="min-w-0 space-y-2">
      <div className="rounded-2xl bg-muted p-3 font-mono text-[13px] leading-6 text-foreground/80">
        <p className="font-medium text-foreground">
          ▸ {t(copy.specLine)} · {spec.stage}
        </p>
        {rows.map((r, i) => (
          <p key={i} className="break-words ps-[2ch]">
            {r}
          </p>
        ))}
        {spec.notes && <p className="break-words ps-[2ch] text-[color:var(--cli-dim)]">{spec.notes}</p>}
      </div>
      {error && (
        <p role="alert" dir="auto" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {canLock && (
        <button
          type="button"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onLock}
          className="inline-flex items-center rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background transition-colors hover:bg-foreground/85"
        >
          {t(copy.lockSpec)}
        </button>
      )}
    </div>
  )
}
