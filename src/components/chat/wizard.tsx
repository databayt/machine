"use client"

import { useEffect, useReducer, useRef } from "react"

import { cn } from "@/lib/utils"
import type { Answered, AskQuestion } from "./ask-pure"
import { copy, tr } from "./config"
import { chatRow, collect, initState, isAnswered, onReview, reduce, tabCount, tabDelta, typeRow, type WAction } from "./wizard-pure"

const MONO = 'ui-monospace, "SF Mono", Menlo, Consolas, monospace'
const accent = "text-[color:var(--cli-accent)]"
const dim = "text-[color:var(--cli-dim)]"

/** A thin full-width rule. */
const Rule = () => <div aria-hidden className="h-px w-full bg-[color:var(--cli-dim)] opacity-60" />

/**
 * The Claude Code CLI's question wizard (AskUserQuestion), drawn the way it looks in a terminal: monospace, thin
 * rules, a filled blue header chip, the question in bold beside a thin gutter bar, numbered options with `❱` and a
 * blue label on the highlighted one, grey descriptions, "Type something.", a rule, "Chat about this", a dim footer
 * and a bottom rule that ends with the chat's title. Several questions get a tab row; a review screen comes last.
 * Keys: ↑/↓ j/k move · Enter selects · 1-9 pick · Space ticks (multi-select) · Tab / Shift+Tab / ←/→ (mirrored in RTL)
 * switch questions · Esc cancels. Clicks work on every row and tab.
 */
export function Wizard({
  questions,
  isAr,
  title,
  onSubmit,
  onCancel,
  onChat,
}: {
  questions: readonly AskQuestion[]
  isAr: boolean
  /** The chat's title, at the end of the bottom rule. */
  title: string
  onSubmit: (answers: Answered[]) => void
  onCancel: () => void
  /** "Chat about this": the wizard closes and the composer takes the focus. */
  onChat: () => void
}) {
  const t = tr(isAr)
  const [s, dispatch] = useReducer(reduce, questions, initState)
  const root = useRef<HTMLDivElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const outcome = useRef({ onSubmit, onCancel, onChat })
  outcome.current = { onSubmit, onCancel, onChat }

  useEffect(() => {
    root.current?.focus({ preventScroll: true })
    root.current?.scrollIntoView({ block: "nearest" })
  }, [])
  useEffect(() => {
    if (s.typing) input.current?.focus()
    else if (!s.done) root.current?.focus({ preventScroll: true })
  }, [s.typing, s.tab, s.done])
  useEffect(() => {
    if (s.done === "submit") outcome.current.onSubmit(collect(s))
    else if (s.done === "cancel") outcome.current.onCancel()
    else if (s.done === "chat") outcome.current.onChat()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once, when it ends
  }, [s.done])

  const act = (a: WAction) => dispatch(a)
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.target instanceof HTMLInputElement || e.metaKey || e.ctrlKey || e.altKey) return
    const k = e.key
    const go = (a: WAction) => {
      e.preventDefault()
      e.stopPropagation()
      act(a)
    }
    if (k === "ArrowUp" || k === "k") return go({ type: "move", delta: -1 })
    if (k === "ArrowDown" || k === "j") return go({ type: "move", delta: 1 })
    if (k === "Enter") return go({ type: "enter" })
    if (k === " ") return go({ type: "space" })
    if (k === "Escape") return go({ type: "esc" })
    if (k === "Tab") return go({ type: "tab", delta: e.shiftKey ? -1 : 1 })
    if (k === "ArrowLeft" || k === "ArrowRight") return tabCount(s) > 1 ? go({ type: "tab", delta: tabDelta(k, isAr) }) : undefined
    if (/^[1-9]$/.test(k)) return go({ type: "digit", n: Number(k) })
  }
  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    const k = e.key
    if (k === "Enter" || k === "Escape" || k === "ArrowUp" || k === "ArrowDown" || (k === "Tab" && tabCount(s) > 1)) {
      e.preventDefault()
      e.stopPropagation()
      if (k === "Enter") act({ type: "enter" })
      else if (k === "Escape") act({ type: "esc" })
      else if (k === "Tab") act({ type: "tab", delta: e.shiftKey ? -1 : 1 })
      else act({ type: "move", delta: k === "ArrowUp" ? -1 : 1 })
    }
  }

  const review = onReview(s)
  const q = review ? null : s.questions[s.tab]!
  const many = tabCount(s) > 1
  const lead = isAr ? "→" : "←"
  const trail = isAr ? "←" : "→"

  return (
    <div
      ref={root}
      role="group"
      aria-label={t(copy.wizLabel)}
      tabIndex={0}
      dir={isAr ? "rtl" : "ltr"}
      onKeyDown={onKeyDown}
      style={{ fontFamily: MONO }}
      className="mt-3 w-full text-[13px] leading-6 text-foreground outline-none"
    >
      <Rule />
      {/* The header chip, or the tab row. */}
      <div className="flex flex-wrap items-center gap-x-1 pt-2">
        {many && (
          <button type="button" tabIndex={-1} aria-label="previous" onClick={() => act({ type: "tab", delta: -1 })} className={cn("px-1", dim)}>
            {lead}
          </button>
        )}
        {s.questions.map((x, i) => {
          const current = !review && s.tab === i
          return (
            <button
              key={i}
              type="button"
              tabIndex={-1}
              onClick={() => act({ type: "goto", tab: i })}
              className={cn("px-2", current ? "bg-[color:var(--cli-accent)] text-white" : "text-foreground")}
            >
              {isAnswered(s, i) ? "✔" : "☐"} {x.header}
            </button>
          )
        })}
        {many && (
          <button
            type="button"
            tabIndex={-1}
            onClick={() => act({ type: "goto", tab: s.questions.length })}
            className={cn("px-2", review ? "bg-[color:var(--cli-accent)] text-white" : "text-foreground")}
          >
            ✔ {t(copy.wizTabSubmit)}
          </button>
        )}
        {many && (
          <button type="button" tabIndex={-1} aria-label="next" onClick={() => act({ type: "tab", delta: 1 })} className={cn("px-1", dim)}>
            {trail}
          </button>
        )}
      </div>

      {review ? (
        <>
          <p className="pt-3 font-bold">{t(copy.wizReview)}</p>
          <ul className="space-y-1 pt-2">
            {s.questions.map((x, i) => {
              const a = isAnswered(s, i) ? collect(s).find((c) => c.question === x.question)?.answer : null
              return (
                <li key={i}>
                  <div>● {x.question}</div>
                  <div className={a ? "ps-3" : cn("ps-3", dim)}>→ {a ?? t(copy.wizNoAnswer)}</div>
                </li>
              )
            })}
          </ul>
          <p className="pt-3 font-bold">{t(copy.wizReady)}</p>
          <div className="pt-1">
            <Row n={1} active={s.cursor === 0} label={t(copy.wizSubmit)} onClick={() => act({ type: "row", row: 0 })} />
            <Row n={2} active={s.cursor === 1} label={t(copy.wizCancel)} onClick={() => act({ type: "row", row: 1 })} />
          </div>
        </>
      ) : (
        q && (
          <>
            <p className="mt-3 border-s-2 border-[color:var(--cli-dim)] ps-2 font-bold">{q.question}</p>
            <div className="pt-2">
              {q.options.map((o, i) => (
                <Row
                  key={i}
                  n={i + 1}
                  active={s.cursor === i}
                  label={o.label}
                  description={o.description}
                  box={q.multiSelect ? (s.checked[s.tab]![i] ? "[✔]" : "[ ]") : undefined}
                  marked={!q.multiSelect && s.picks[s.tab] === i}
                  onClick={() => act({ type: "row", row: i })}
                />
              ))}
              <Row
                n={typeRow(q) + 1}
                active={s.cursor === typeRow(q)}
                label={s.typing ? "" : s.typed[s.tab]!.trim() || t(copy.wizTypeSomething)}
                onClick={() => act({ type: "row", row: typeRow(q) })}
              >
                {s.typing && (
                  <input
                    ref={input}
                    value={s.typed[s.tab]}
                    onChange={(e) => act({ type: "type", text: e.target.value })}
                    onKeyDown={onInputKeyDown}
                    dir="auto"
                    spellCheck={false}
                    maxLength={500}
                    aria-label={t(copy.wizTypeSomething)}
                    placeholder={t(copy.wizTypeSomething)}
                    className="min-w-0 flex-1 bg-transparent font-[inherit] text-[color:var(--cli-accent)] outline-none placeholder:text-[color:var(--cli-dim)]"
                  />
                )}
              </Row>
            </div>
            <div className="py-1">
              <Rule />
            </div>
            <Row n={chatRow(q) + 1} active={s.cursor === chatRow(q)} label={t(copy.wizChat)} onClick={() => act({ type: "row", row: chatRow(q) })} />
          </>
        )
      )}

      <p className={cn("pt-3", dim)}>
        {q?.multiSelect ? t(copy.wizFooterMulti) : t(copy.wizFooter)}
        {many ? t(copy.wizFooterTabs) : ""}
      </p>
      <div className={cn("flex items-center gap-2 pt-1", dim)}>
        <div aria-hidden className="h-px flex-1 bg-[color:var(--cli-dim)] opacity-60" />
        <span className="max-w-[60%] truncate" dir="auto">
          {title}
        </span>
        <div aria-hidden className="h-px w-3 bg-[color:var(--cli-dim)] opacity-60" />
      </div>
    </div>
  )
}

/** One numbered row. The highlighted one carries `❱` and a blue label; the description sits under the label, indented. */
function Row({
  n,
  active,
  label,
  description,
  box,
  marked,
  onClick,
  children,
}: {
  n: number
  active: boolean
  label: string
  description?: string
  box?: string
  marked?: boolean
  onClick: () => void
  children?: React.ReactNode
}) {
  return (
    <div
      role="button"
      tabIndex={-1}
      onClick={onClick}
      className="flex cursor-pointer items-start"
    >
      <span aria-hidden className={cn("w-[2ch] shrink-0", accent)}>
        {active ? "❱" : ""}
      </span>
      <div className="min-w-0 flex-1">
        <div className={cn("flex items-baseline gap-[1ch]", active && accent)}>
          {box && <span className="shrink-0 whitespace-pre">{box}</span>}
          <span className="shrink-0">{n}.</span>
          {children ?? <span className="min-w-0 break-words">{label}{marked ? " ✔" : ""}</span>}
        </div>
        {description && <div className={cn("break-words text-foreground/75", box ? "ps-[7ch]" : "ps-[3ch]")}>{description}</div>}
      </div>
    </div>
  )
}
