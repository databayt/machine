"use client"

import { useEffect, useReducer, useRef } from "react"

import { cn } from "@/lib/utils"
import type { Question } from "../pack/types"
import { copy, tr } from "./config"
import {
  type QState,
  answered,
  answersToSet,
  initState,
  optionLabel,
  reduce,
  shownValue,
  slotTitle,
  tabDelta,
} from "./questions-pure"

/**
 * The question round before Send, in Claude Code's AskUserQuestion look: a muted slot chip, the question
 * in semibold, numbered options with ❯ on the highlighted row and ✔ after the current value, and a last
 * "Type something…" row. Several questions get a tab row (☐ Market  ✔ Length  Submit).
 * Keys: ↑/↓ move · Enter or 1-9 picks and advances · Tab and ←/→ (mirrored in RTL) change tab · Esc keeps the defaults.
 * No model runs here; `onDone` gets the answers to merge into the fill (empty after Esc).
 */
export function QuestionCard({
  questions,
  isAr,
  onDone,
}: {
  questions: readonly Question[]
  isAr: boolean
  onDone: (answers: Record<string, string>, submitted: boolean) => void
}) {
  const t = tr(isAr)
  const [s, dispatch] = useReducer(reduce, questions, initState)
  const root = useRef<HTMLDivElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const sent = useRef(false)

  // Focus moves into the card when it opens.
  useEffect(() => {
    root.current?.focus()
  }, [])
  // Typing: the input takes the focus; leaving it hands the focus back to the card.
  useEffect(() => {
    if (s.typing) input.current?.focus()
    else root.current?.focus()
  }, [s.typing, s.tab])
  useEffect(() => {
    if (!s.done || sent.current) return
    sent.current = true
    onDone(answersToSet(s), s.done === "submit")
  }, [s, onDone])

  const q = s.questions[s.tab]
  const multi = s.questions.length > 1
  const onSubmit = multi && s.tab >= s.questions.length

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return
    const k = e.key
    const stop = () => {
      e.preventDefault()
      e.stopPropagation()
    }
    if (k === "Escape") return stop(), dispatch({ type: "esc" })
    if (k === "Tab") return stop(), dispatch({ type: "tab", delta: e.shiftKey ? -1 : 1 })
    if (k === "Enter") return stop(), dispatch({ type: "enter" })
    if (k === "ArrowUp") return stop(), dispatch({ type: "move", delta: -1 })
    if (k === "ArrowDown") return stop(), dispatch({ type: "move", delta: 1 })
    if ((k === "ArrowLeft" || k === "ArrowRight") && !s.typing) return stop(), dispatch({ type: "tab", delta: tabDelta(k, isAr) })
    if (!s.typing && /^[1-9]$/.test(k)) return stop(), dispatch({ type: "digit", n: Number(k) })
  }

  const tabs = multi && (
    <div className="mb-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-sm" role="tablist">
      {s.questions.map((x, i) => (
        <button
          key={x.slot}
          type="button"
          role="tab"
          tabIndex={-1}
          aria-selected={s.tab === i}
          onClick={() => dispatch({ type: "goto", tab: i })}
          className={cn("flex items-center gap-1.5 rounded-md px-1.5 py-0.5", s.tab === i ? "bg-muted font-semibold text-foreground" : "text-muted-foreground hover:text-foreground")}
        >
          <span aria-hidden>{answered(s, i) ? "✔" : "☐"}</span>
          {slotTitle(x.slot, isAr)}
        </button>
      ))}
      <button
        type="button"
        role="tab"
        tabIndex={-1}
        aria-selected={onSubmit}
        onClick={() => dispatch({ type: "goto", tab: s.questions.length })}
        className={cn("flex items-center gap-1.5 rounded-md px-1.5 py-0.5", onSubmit ? "bg-muted font-semibold text-foreground" : "text-muted-foreground hover:text-foreground")}
      >
        <span aria-hidden>✔</span>
        {t(copy.qSubmit)}
      </button>
    </div>
  )

  return (
    <div
      ref={root}
      tabIndex={0}
      role="group"
      aria-label={t(copy.qLabel)}
      onKeyDown={onKeyDown}
      className="mb-3 w-full rounded-2xl border border-border/60 bg-background p-4 text-start shadow-[0_4px_16px_-10px_rgb(0_0_0/0.10)] outline-none focus-visible:border-border"
    >
      {tabs}
      {onSubmit ? (
        <SubmitTab s={s} isAr={isAr} onPick={() => dispatch({ type: "enter" })} />
      ) : (
        q && <QuestionRows q={q} s={s} isAr={isAr} dispatch={dispatch} input={input} />
      )}
      <p className="mt-3 text-xs text-muted-foreground" dir="auto">
        {t(copy.qKeys)}
      </p>
    </div>
  )
}

function QuestionRows({
  q,
  s,
  isAr,
  dispatch,
  input,
}: {
  q: Question
  s: QState
  isAr: boolean
  dispatch: (a: Parameters<typeof reduce>[1]) => void
  input: React.RefObject<HTMLInputElement | null>
}) {
  const t = tr(isAr)
  const shown = shownValue(q, s.answers)
  const typeRow = q.options.length
  const rowBase = "flex w-full items-baseline gap-2 rounded-lg px-2 py-1.5 text-start text-[15px] leading-6"
  const mark = (on: boolean) => (
    <span aria-hidden className={cn("w-4 shrink-0 text-center text-claude", !on && "invisible")} dir="ltr">
      ❯
    </span>
  )
  return (
    <div>
      <span className="inline-block rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground">{slotTitle(q.slot, isAr)}</span>
      <p className="mb-2 mt-2 text-base font-semibold" dir="auto">
        {isAr ? q.q.ar || q.q.en : q.q.en || q.q.ar}
      </p>
      <ul role="listbox" className="m-0 list-none p-0">
        {q.options.map((o, i) => {
          const on = s.cursor === i && !s.typing
          return (
            <li key={`${i}-${o.value}`}>
              <button
                type="button"
                role="option"
                aria-selected={shown === o.value}
                tabIndex={-1}
                onMouseMove={() => s.cursor !== i && dispatch({ type: "move", delta: i - s.cursor })}
                onClick={() => dispatch({ type: "row", row: i })}
                className={cn(rowBase, on ? "bg-muted text-foreground" : "text-muted-foreground hover:bg-muted/60")}
              >
                {mark(on)}
                <span className="tabular-nums" dir="ltr">{i + 1}.</span>
                <span className="min-w-0" dir="auto">{optionLabel(o, isAr)}</span>
                {shown === o.value && (
                  <span aria-label={t(copy.qCurrent)} className="text-claude" dir="ltr">
                    ✔
                  </span>
                )}
              </button>
            </li>
          )
        })}
        <li>
          <div
            role="option"
            aria-selected={false}
            onClick={() => dispatch({ type: "row", row: typeRow })}
            className={cn(rowBase, s.cursor === typeRow ? "bg-muted text-foreground" : "text-muted-foreground hover:bg-muted/60")}
          >
            {mark(s.cursor === typeRow)}
            <span className="tabular-nums" dir="ltr">{typeRow + 1}.</span>
            {s.typing ? (
              <input
                ref={input}
                value={s.typed}
                onChange={(e) => dispatch({ type: "type", text: e.target.value })}
                placeholder={t(copy.qTypeSomething)}
                dir="auto"
                className="min-w-0 flex-1 bg-transparent text-foreground outline-none placeholder:text-muted-foreground"
              />
            ) : (
              <span dir="auto">{s.answers[q.slot] !== undefined && !q.options.some((o) => o.value === s.answers[q.slot]) ? s.answers[q.slot] : t(copy.qTypeSomething)}</span>
            )}
          </div>
        </li>
      </ul>
    </div>
  )
}

function SubmitTab({ s, isAr, onPick }: { s: QState; isAr: boolean; onPick: () => void }) {
  const t = tr(isAr)
  return (
    <div>
      <ul className="m-0 mb-3 list-none space-y-1 p-0 text-[15px]">
        {s.questions.map((x) => {
          const v = shownValue(x, s.answers)
          const opt = x.options.find((o) => o.value === v)
          return (
            <li key={x.slot} className="flex items-baseline gap-2">
              <span className="text-muted-foreground">{slotTitle(x.slot, isAr)}</span>
              <span dir="auto" className="font-medium">{opt ? optionLabel(opt, isAr) : (v ?? "—")}</span>
            </li>
          )
        })}
      </ul>
      <button
        type="button"
        tabIndex={-1}
        onClick={onPick}
        className="inline-flex items-center rounded-full bg-foreground px-4 py-1.5 text-sm font-medium text-background transition-colors hover:bg-foreground/85"
      >
        <span aria-hidden className="me-2 text-claude" dir="ltr">❯</span>
        {t(copy.qSubmit)}
      </button>
      <span className="ms-3 text-xs text-muted-foreground">{t(copy.qSubmitHint)}</span>
    </div>
  )
}

