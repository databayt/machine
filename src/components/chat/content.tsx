"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { ArrowDownIcon, InfoIcon, RotateCcwIcon } from "lucide-react";

import type { Locale } from "@/components/local/config";
import type { Event } from "../driver/types";
import { board as loadBoard, fill as fillFormula, formulas as loadFormulas, matchText, packInfo, questions as loadQuestions, stationLog } from "../pack/actions";
import type { Board, FormulaMeta, Match, PackInfo, Question } from "../pack/types";
import type { CatalogItem } from "../catalog/types";
import { loadCatalog } from "../menu/data";
import { CHAT_DELETED_EVENT, NEW_CHAT_EVENT } from "../menu/config";
import { useReadAloud } from "../menu/use-read-aloud";
import { getSession } from "../sessions/actions";
import { OpenInMachine, useInMachine } from "../shell/content";
import { storedLang, swapLocale } from "../shell/lang";
import { SubmitWork } from "../tasks/submit-work";
import { speak } from "../voice/actions";
import { acceptMessage, orderMessage, showAccept, workingRoute } from "./accept";
import { ask, refine, stop } from "./actions";
import { ActivityRows, ClaudeStatus } from "./activity";
import { splitNote } from "./activity-pure";
import { QuestionCard } from "./questions";
import { type Family, effectiveModel, familyOf } from "./model-pure";
import { Composer, type Fill } from "./composer";
import { ContextLine, useWorkingContext } from "./context";
import { copy, greeting, tr } from "./config";
import { type Chip, type SlashEntry, chipFacts, matchSkills, packIds } from "./keywords";
import { type HomeOption, homeOptions, usableMatch } from "./options";
import { ModelChip } from "../brain/chip";
import { parseRefined } from "./refine";
import { Suggestions } from "./suggestions";
import { useDeptHealth } from "../pack/health";
import { AgentTurn, Prose, TurnActions, UserTurn } from "./turn";
import { useStickToBottom } from "./use-stick-to-bottom";

interface Message {
  role: "user" | "assistant";
  text: string;
  /** Finished tool steps, in order. */
  tools: string[];
  /** The tool running now. */
  live: string | null;
  streaming?: boolean;
  error?: string;
  /** The route the turn ran on (null: none was named). */
  route?: string | null;
  /** The turn ran in plan mode: it ends with Accept. */
  plan?: boolean;
  /** The `record` of the formula that opened the plan (`issue` or `none`). */
  record?: string | null;
}

/**
 * The machine's home — the chat in mkan's /find shell, with no bar: the greeting mid-screen with
 * starter cards above a pill composer pinned to the bottom, user bubbles, plain agent text, folded tool
 * steps and an orange working line. The mega menu (layout) holds chats, lanes and settings.
 */
export default function ChatContent({
  lang,
  session: resume,
  prompt,
  task,
}: {
  lang: Locale;
  session?: string;
  prompt?: string;
  /** A started task (`databayt/<repo>#<n>`): its runs get TASK.md and the task's repos. */
  task?: string;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const router = useRouter();
  const inApp = useInMachine();
  const [pack, setPack] = useState<PackInfo | null>(null);
  const ctx = useWorkingContext(inApp === true);
  const health = useDeptHealth(inApp === true ? ctx.dept : "");
  const [messages, setMessages] = useState<Message[]>([]);
  const [session, setSession] = useState<string | null>(resume ?? null);
  const [resumed, setResumed] = useState<string | null>(null);
  const [jobId, setJobId] = useState<string | null>(null);
  const [speakReplies] = useReadAloud();
  /** The brief an option or ✨ writes into the home composer. */
  const [fill, setFill] = useState<Fill | null>(null);
  /** The station's departure board for the department and repo (the key says whose it is). */
  const [board, setBoard] = useState<{ key: string; value: Board } | null>(null);
  /** The formula the box was last filled from: its `record` says what accepting the plan creates. */
  const stationRef = useRef<{ id: string; record: string } | null>(null);
  /** A route to keep across the silent department switch ✨ may make. */
  const carry = useRef<{ dept: string; route: string | null } | null>(null);
  /** The composer's text, for a held task's Submit. */
  const draft = useRef("");
  const onDraft = useCallback((text: string) => {
    draft.current = text;
  }, []);
  /** Where the next prompt runs, silently: the option's or refine's route, else the pack default. */
  const [routeId, setRouteId] = useState<string | null>(null);
  const fillN = useRef(0);
  /** The question round open above the composer: the formula's open slots, and the set its fill used. */
  const [qcard, setQcard] = useState<{ n: number; dept: string; id: string; set: Record<string, string>; questions: Question[] } | null>(null);
  const qN = useRef(0);
  /** What the formula asked for (model, effort) and the chip's one-run pick; each is spent by the next send. */
  const [formulaModel, setFormulaModel] = useState<string | null>(null);
  const [formulaEffort, setFormulaEffort] = useState<string | null>(null);
  const [override, setOverride] = useState<Family | null>(null);
  const runModel = useRef<{ model: Family | null; effort: string | null }>({ model: null, effort: null });
  runModel.current = { model: override ?? familyOf(formulaModel), effort: formulaEffort };
  const speakRef = useRef(false);
  speakRef.current = speakReplies;

  // The language chosen in Settings opens the app next time: a stored one that differs from the URL wins.
  useEffect(() => {
    const stored = storedLang();
    if (stored && stored !== lang)
      router.replace(
        swapLocale(window.location.pathname, window.location.search, stored),
      );
  }, [lang, router]);

  // The department's pack follows Settings live (the menu, chat and sidebar share one department).
  useEffect(() => {
    if (!inApp || !ctx.dept) return;
    void packInfo(ctx.dept).then(setPack).catch(() => setPack(null));
  }, [inApp, ctx.dept]);

  // The catalog (skills and their keywords) and the department's formulas feed the "/" menu and the keyword chip.
  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  const [formulaList, setFormulaList] = useState<{ dept: string; list: FormulaMeta[] }>({ dept: "", list: [] });
  const probeN = useRef(0);
  useEffect(() => {
    if (inApp === true) void loadCatalog().then(setCatalog);
  }, [inApp]);
  useEffect(() => {
    if (!inApp || !ctx.dept) return;
    let live = true;
    void loadFormulas(ctx.dept).then((list) => live && setFormulaList({ dept: ctx.dept, list }));
    return () => {
      live = false;
    };
  }, [inApp, ctx.dept]);
  const hidden = ctx.profile?.hidden;
  const skillMatches = useCallback(
    (text: string) => matchSkills(text, catalog, ctx.dept, hidden ?? []),
    [catalog, ctx.dept, hidden],
  );
  const slashEntries = useMemo<SlashEntry[]>(() => {
    const formulas: SlashEntry[] = (formulaList.dept === ctx.dept ? formulaList.list : []).map((f) => ({
      kind: "formula",
      id: f.id,
      name: f.id,
      title: (isAr ? f.label_ar || f.label_en : f.label_en || f.label_ar) || f.id,
      hint: f.intent,
      keywords: [f.label_en, f.label_ar],
      group: 0,
      icon: f.icon,
    }));
    const kit = packIds(catalog, ctx.dept);
    const skills: SlashEntry[] = catalog
      .filter((i) => i.id.startsWith("skill:") && !(hidden ?? []).some((h) => h === i.id || h === i.name))
      .map((i) => ({
        kind: "skill",
        id: i.id,
        name: i.name,
        title: (isAr ? i.title_ar || i.title : i.title) || i.name,
        hint: (isAr ? i.description_ar || i.description : i.description) ?? "",
        keywords: i.keywords,
        group: kit.has(i.id) ? 1 : 2,
      }));
    return [...formulas, ...skills.sort((a, b) => a.group - b.group || a.name.localeCompare(b.name))];
  }, [formulaList, catalog, ctx.dept, hidden, isAr]);

  const routes = pack?.routes ?? [];
  const boardKey = `${ctx.dept}|${ctx.repo}`;
  const level = health.health?.level ?? "none";

  // A new chat (or another department) starts on the pack's default route, unless ✨ just moved the
  // department on purpose and brought its route along.
  const isEmpty = messages.length === 0;
  useEffect(() => {
    if (!isEmpty) return;
    const kept = carry.current;
    if (kept && kept.dept === ctx.dept) {
      carry.current = null;
      setRouteId(kept.route);
    } else setRouteId(null);
  }, [ctx.dept, isEmpty]);

  // The departure board: asked when the home shows, and again when the repo or the health light changes.
  // It never blocks the home: an empty answer means the pack's quick actions show instead.
  useEffect(() => {
    if (!inApp || !ctx.dept || !isEmpty) return;
    let live = true;
    void loadBoard(ctx.dept, ctx.repo || null, level === "none" ? null : level).then((value) => {
      if (live) setBoard({ key: boardKey, value });
    });
    return () => {
      live = false;
    };
  }, [inApp, ctx.dept, ctx.repo, level, isEmpty, boardKey]);

  useEffect(() => {
    if (!inApp || !resume) return;
    void getSession(resume)
      .then((s) => setResumed(s?.title ?? null))
      .catch(() => setResumed(null));
  }, [inApp, resume]);

  const busy = jobId !== null;
  const last = messages.at(-1);
  const {
    ref: scrollRef,
    onScroll,
    away,
    scrollToBottom,
  } = useStickToBottom(
    `${messages.length}.${last?.text.length ?? 0}.${last?.tools.length ?? 0}.${last?.live}`,
  );

  const patchLast = useCallback((f: (m: Message) => Message) => {
    setMessages((all) => all.map((m, i) => (i === all.length - 1 ? f(m) : m)));
  }, []);

  const onEvent = useCallback(
    (e: Event) => {
      // A tool still running when the next thing arrives has finished.
      const settle = (m: Message): Message =>
        m.live ? { ...m, tools: [...m.tools, m.live], live: null } : m;
      switch (e.kind) {
        case "started":
          setSession(e.session);
          break;
        case "text":
          patchLast((m) => ({
            ...settle(m),
            text: m.text ? `${m.text}\n\n${e.text}` : e.text,
          }));
          break;
        case "tool":
          patchLast((m) => ({ ...settle(m), live: e.name }));
          break;
        case "done":
          if (e.session) setSession(e.session);
          if (speakRef.current && e.ok && e.result)
            void speak(e.result, isAr ? "ar" : "en").catch(() => {});
          patchLast((m) => ({
            ...settle(m),
            streaming: false,
            text: m.text || e.result || "",
            error:
              e.ok || e.result === "stopped" ? undefined : t(copy.runError),
          }));
          break;
        case "error":
          patchLast((m) => ({
            ...settle(m),
            streaming: false,
            error: e.message,
          }));
          break;
      }
    },
    [patchLast, isAr, t],
  );

  const send = useCallback(
    async (prompt: string, base: Message[] = messages, route?: string | null) => {
      const text = prompt.trim();
      if (!text || busy) return;
      setFill(null);
      qN.current++;
      setQcard(null);
      const { model, effort } = runModel.current;
      setOverride(null);
      setFormulaModel(null);
      setFormulaEffort(null);
      const id = crypto.randomUUID();
      setJobId(id);
      const ranInPlan = routes.find((r) => r.id === route)?.mode === "plan";
      setMessages([
        ...base,
        { role: "user", text, tools: [], live: null },
        {
          role: "assistant",
          text: "",
          tools: [],
          live: null,
          streaming: true,
          route: route ?? null,
          plan: ranInPlan,
          record: ranInPlan ? (stationRef.current?.record ?? null) : null,
        },
      ]);
      requestAnimationFrame(() => scrollToBottom());
      try {
        await ask(text, { jobId: id, session, taskId: task ?? null, repo: ctx.repo || null, route: route ?? null, model, effort }, onEvent);
      } catch (err) {
        patchLast((m) => ({ ...m, streaming: false, error: String(err) }));
      } finally {
        patchLast((m) => ({
          ...m,
          streaming: false,
          tools: m.live ? [...m.tools, m.live] : m.tools,
          live: null,
        }));
        setJobId(null);
      }
    },
    [messages, busy, session, task, ctx.repo, routes, onEvent, patchLast, scrollToBottom],
  );

  /** Runs the last prompt again, in place of the failed answer. */
  const retry = () => {
    const i = messages.findLastIndex((m) => m.role === "user");
    if (i >= 0) void send(messages[i]!.text, messages.slice(0, i), messages[i + 1]?.route ?? null);
  };

  const newChat = () => {
    if (busy) return;
    setMessages([]);
    setSession(null);
    setResumed(null);
    stationRef.current = null;
    qN.current++;
    setQcard(null);
    if (resume) router.replace(`/${lang}/`);
  };

  // The menu's "New chat" and its delete both reach the open chat through window events.
  const newChatRef = useRef(newChat);
  newChatRef.current = newChat;
  const sessionRef = useRef(session);
  sessionRef.current = session;
  useEffect(() => {
    const onNew = () => newChatRef.current();
    const onDeleted = (e: globalThis.Event) => {
      if ((e as CustomEvent<string>).detail === sessionRef.current)
        newChatRef.current();
    };
    window.addEventListener(NEW_CHAT_EVENT, onNew);
    window.addEventListener(CHAT_DELETED_EVENT, onDeleted);
    return () => {
      window.removeEventListener(NEW_CHAT_EVENT, onNew);
      window.removeEventListener(CHAT_DELETED_EVENT, onDeleted);
    };
  }, []);

  if (inApp === null) return null;
  if (!inApp) return <OpenInMachine lang={lang} />;

  const empty = messages.length === 0;
  const options: HomeOption[] =
    board?.key === boardKey ? homeOptions(board.value, pack?.quick_actions ?? [], ctx.repo, isAr) : [];
  const chosen =
    routes.find((r) => r.id === routeId) ??
    routes.find((r) => r.id === pack?.default_route) ??
    routes[0] ??
    null;
  /** The route that carries work on: the first that is not plan mode. */
  const working = workingRoute(routes);
  /** Sends on the chosen route. A reply to a plan stays on the plan route: it answers or changes the plan. */
  const sendRouted = (text: string) => {
    // A filled work order is for someone else: sending it asks the lead to create the issue, not to do the work.
    const order = stationRef.current?.record === "issue" ? stationRef.current : null;
    void send(order ? orderMessage(text, order.id) : text, undefined, chosen?.id ?? null);
    if (order) stationRef.current = null;
  };
  /** Accept: the plan becomes work. Only this switches to the working route. */
  const accept = (record: string | null | undefined) => {
    if (!working) return;
    setRouteId(working.id);
    void send(acceptMessage(record), undefined, working.id);
  };
  /** Writes a filled prompt into the box, caret at the end, nothing sent; the route is set silently. */
  const writeFilled = (
    text: string,
    route: string | null,
    formula: { id: string; record: string } | null,
    run: { model?: string | null; effort?: string | null } = {},
  ) => {
    stationRef.current = formula;
    setFormulaModel(run.model ?? null);
    setFormulaEffort(run.effort ?? null);
    setFill({ text, focus: true, n: ++fillN.current });
    if (route) setRouteId(route);
  };
  /** The question round after a fill: the formula's open slots, asked with no model. `set` is the one the fill used. */
  const openQuestions = (dept: string, id: string, set: Record<string, string>) => {
    const n = ++qN.current;
    setQcard(null);
    void loadQuestions(dept, id, ctx.repo || null, set).then((r) => {
      if (n === qN.current && r.questions.length > 0) setQcard({ n, dept, id: r.id || id, set, questions: r.questions });
    });
  };
  /** The card is over: Submit fills again with the answers merged over the first set (the prompt, never sent); Esc keeps the defaults. */
  const finishQuestions = async (card: NonNullable<typeof qcard>, answers: Record<string, string>, submitted: boolean) => {
    qN.current++;
    setQcard(null);
    if (submitted && Object.keys(answers).length > 0) {
      try {
        const f = await fillFormula(card.dept, card.id, ctx.repo || null, { ...card.set, ...answers });
        writeFilled(f.prompt, f.route, { id: f.id, record: f.record }, f);
        return;
      } catch {
        // the old prompt stays
      }
    }
    setFill({ text: draft.current, focus: true, n: ++fillN.current });
  };
  /** A tap: the station fills the formula at once (no model); a quick action writes its brief and refines. */
  const pickOption = async (o: HomeOption) => {
    if (o.logId) void stationLog(ctx.dept, ctx.repo || null, o.logId, "pick");
    if (o.formula) {
      try {
        const f = await fillFormula(ctx.dept, o.formula, ctx.repo || null, o.set);
        writeFilled(f.prompt, f.route ?? o.route, { id: f.id, record: f.record }, f);
        openQuestions(ctx.dept, f.id, o.set);
      } catch {
        // The station's script is unavailable: the option's own English name is the request.
        writeFilled(o.labelEn, o.route, null);
      }
      return;
    }
    stationRef.current = null;
    setFill({ text: o.prompt ?? "", focus: true, refine: routes.length > 0 && o.refine, n: ++fillN.current });
    if (o.route && routes.some((r) => r.id === o.route)) setRouteId(o.route);
  };
  const dismissOption = (o: HomeOption) => {
    if (o.logId) void stationLog(ctx.dept, ctx.repo || null, o.logId, "dismiss");
  };
  /**
   * A station match applied: the department switches silently when the text names another, the formula
   * is filled (no model), its route and record are kept, and the pick is logged. The prompt, or null.
   */
  const applyMatch = async (m: Match & { id: string }): Promise<string | null> => {
    let dept = ctx.dept;
    if (m.dept && m.dept !== ctx.dept && (await ctx.chooseDept(m.dept))) dept = m.dept;
    try {
      const f = await fillFormula(dept, m.id, ctx.repo || null, m.set ?? {});
      if (dept !== ctx.dept) carry.current = { dept, route: f.route };
      else if (f.route) setRouteId(f.route);
      stationRef.current = { id: f.id, record: f.record };
      setFormulaModel(f.model);
      setFormulaEffort(f.effort);
      void stationLog(dept, ctx.repo || null, f.id, "pick");
      openQuestions(dept, f.id, m.set ?? {});
      return f.prompt;
    } catch {
      return null;
    }
  };
  /** ✨: the station's switch first; with an id and enough confidence the formula is filled with no model run. */
  const refineText = async (text: string) => {
    const m = await matchText(ctx.dept, text, ctx.repo || null);
    if (usableMatch(m)) {
      const prompt = await applyMatch({ ...m, id: m.id });
      if (prompt !== null) return prompt;
      // fall through to the model
    }
    const raw = await refine(text, ctx.repo || null);
    const out = parseRefined(raw, routes.map((r) => r.id));
    if (!out.text) return null;
    stationRef.current = null;
    if (out.route) setRouteId(out.route);
    return out.text;
  };
  /** The composer's live match: the station's formula when it is sure, else a skill the text names. */
  const probe = async (text: string): Promise<Chip | null> => {
    const n = ++probeN.current;
    const m = await matchText(ctx.dept, text, ctx.repo || null);
    if (n !== probeN.current) return null;
    if (usableMatch(m)) {
      const meta = formulaList.list.find((f) => f.id === m.id);
      const label = (meta && (isAr ? meta.label_ar || meta.label_en : meta.label_en || meta.label_ar)) || m.id;
      return { kind: "formula", forText: text, id: m.id, label, facts: chipFacts(m.set), match: { ...m, id: m.id } };
    }
    const top = skillMatches(text)[0];
    return top ? { kind: "skill", forText: text, name: top.name } : null;
  };
  const acceptChip = (c: Extract<Chip, { kind: "formula" }>) => applyMatch(c.match);
  const dismissChip = (c: Chip) => {
    if (c.kind === "formula") void stationLog(ctx.dept, ctx.repo || null, c.id, "dismiss");
  };
  /** A formula chosen from the "/" menu: filled with no slots, so the open ones are for the person. */
  const pickFormula = async (id: string): Promise<string | null> => {
    try {
      const f = await fillFormula(ctx.dept, id, ctx.repo || null, {});
      if (f.route) setRouteId(f.route);
      stationRef.current = { id: f.id, record: f.record };
      setFormulaModel(f.model);
      setFormulaEffort(f.effort);
      void stationLog(ctx.dept, ctx.repo || null, f.id, "pick");
      openQuestions(ctx.dept, f.id, {});
      return f.prompt;
    } catch {
      return null;
    }
  };
  /** A held work order's Submit: clear the box, or put the caret in it to ask for the summary line. */
  const afterSubmit = () => setFill({ text: "", focus: false, n: ++fillN.current });
  const askSummary = () => setFill({ text: draft.current, focus: true, n: ++fillN.current });
  const questionCard =
    qcard && !busy ? (
      <QuestionCard
        key={qcard.n}
        questions={qcard.questions}
        isAr={isAr}
        onDone={(answers, submitted) => void finishQuestions(qcard, answers, submitted)}
      />
    ) : null;
  const modelChip = (
    <ModelChip
      lang={lang}
      disabled={busy}
      session={session}
      model={effectiveModel(null, formulaModel, chosen?.model ?? null)}
      override={override}
      onOverride={setOverride}
    />
  );
  const contextLine = (
    <ContextLine
      lang={lang}
      profile={ctx.profile}
      dept={ctx.dept}
      onDept={ctx.chooseDept}
      repos={ctx.repos}
      local={ctx.local}
      repo={ctx.repo}
      onRepo={ctx.setRepo}
      health={health}
      onAsk={(p) => void send(p, undefined, working?.id ?? null)}
      disabled={busy}
    />
  );

  return (
    <div className="flex h-full flex-col overflow-x-clip">
      <div className="relative flex min-h-0 flex-1 flex-col">
        <div
          ref={scrollRef}
          onScroll={onScroll}
          className="relative flex-1 overflow-y-auto overflow-x-hidden overscroll-contain"
        >
          {empty ? (
            <div className="mx-auto flex min-h-full w-full max-w-3xl flex-col items-center justify-center gap-8 px-6 pb-[12vh] pt-20 lg:px-10 lg:pt-24">
              <div className="text-center">
                <h1
                  className={`text-balance font-['thmanyah_serif_display',var(--font-sans)] font-bold leading-snug ${isAr ? "text-3xl sm:text-4xl" : "text-xl tracking-tight sm:text-2xl"}`}
                >
                  {greeting(isAr)}
                </h1>
                {resumed && (
                  <p className="mt-2 text-sm text-muted-foreground">
                    {t(copy.continuing)}: {resumed}
                  </p>
                )}
              </div>
              <div className="w-full">
                {questionCard}
                <Composer
                  lang={lang}
                  onSend={sendRouted}
                  onStop={() => jobId && void stop(jobId)}
                  busy={busy}
                  initialText={prompt}
                  fill={fill}
                  repo={ctx.repo}
                  onRefine={routes.length ? refineText : undefined}
                  onDraft={onDraft}
                  onProbe={probe}
                  onAcceptChip={acceptChip}
                  onDismissChip={dismissChip}
                  skillMatches={skillMatches}
                  slashEntries={slashEntries}
                  onPickFormula={pickFormula}
                  autoFocus
                />
                {modelChip}
                {contextLine}
                {task && (
                  <SubmitWork lang={lang} task={task} draft={() => draft.current} onDone={afterSubmit} onAskSummary={askSummary} />
                )}
              </div>
              <Suggestions
                options={options}
                isAr={isAr}
                onPick={(o) => void pickOption(o)}
                onDismiss={dismissOption}
                disabled={busy}
              />
            </div>
          ) : (
            <ol className="mx-auto w-full max-w-3xl list-none space-y-7 px-6 pb-8 pt-20 lg:px-10 lg:pt-24">
              {messages.map((m, i) => {
                const note = splitNote(m.text);
                return m.role === "user" ? (
                  <UserTurn key={i} text={m.text} />
                ) : (
                  <AgentTurn
                    key={i}
                    steps={
                      <ActivityRows done={m.tools} live={m.live} isAr={isAr} />
                    }
                  >
                    {note.note && (
                      <p dir="auto" className="flex items-start gap-2 text-sm text-muted-foreground">
                        <InfoIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
                        <span>{note.note}</span>
                      </p>
                    )}
                    {note.rest && <Prose text={note.rest} />}
                    {m.error && (
                      <p
                        role="alert"
                        className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-destructive"
                      >
                        <span dir="auto">{m.error}</span>
                        {i === messages.length - 1 && !busy && (
                          <button
                            type="button"
                            onClick={retry}
                            className="inline-flex items-center gap-1.5 rounded-full bg-muted px-3 py-1 font-medium text-foreground transition-colors hover:bg-muted/70"
                          >
                            <RotateCcwIcon className="size-3.5" aria-hidden />
                            {t(copy.retry)}
                          </button>
                        )}
                      </p>
                    )}
                    {m.streaming && <ClaudeStatus isAr={isAr} seed={String(i)} onStop={() => jobId && void stop(jobId)} />}
                    {!m.streaming && m.text && !m.error && (
                      <TurnActions text={m.text} isAr={isAr} />
                    )}
                    {showAccept({
                      ranInPlan: !!m.plan,
                      last: i === messages.length - 1,
                      busy,
                      streaming: !!m.streaming,
                      hasText: !!m.text,
                      failed: !!m.error,
                      hasWorkingRoute: !!working,
                    }) && (
                      <button
                        type="button"
                        onClick={() => accept(m.record)}
                        className="inline-flex items-center rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background transition-colors hover:bg-foreground/85"
                      >
                        {t(copy.acceptPlan)}
                      </button>
                    )}
                  </AgentTurn>
                );
              })}
            </ol>
          )}
        </div>
      </div>

      {!empty && (
        <div className="relative mx-auto w-full max-w-3xl px-6 pb-[max(env(safe-area-inset-bottom),20px)] lg:px-10 lg:pb-8">
          {away && (
            <button
              type="button"
              onClick={() => scrollToBottom()}
              aria-label={t(copy.scrollDown)}
              className="absolute -top-14 start-1/2 flex size-10 -translate-x-1/2 items-center justify-center rounded-full border border-border/60 bg-background shadow-[0_2px_10px_rgba(0,0,0,0.12)] rtl:translate-x-1/2"
            >
              <ArrowDownIcon className="size-5" aria-hidden />
            </button>
          )}
          {questionCard}
          <Composer
            lang={lang}
            onSend={sendRouted}
            onStop={() => jobId && void stop(jobId)}
            busy={busy}
            fill={fill}
            repo={ctx.repo}
            onRefine={routes.length ? refineText : undefined}
            onDraft={onDraft}
            onProbe={probe}
            onAcceptChip={acceptChip}
            onDismissChip={dismissChip}
            skillMatches={skillMatches}
            slashEntries={slashEntries}
            onPickFormula={pickFormula}
            autoFocus
          />
          {modelChip}
          {contextLine}
          {task && (
            <SubmitWork lang={lang} task={task} draft={() => draft.current} onDone={afterSubmit} onAskSummary={askSummary} />
          )}
          <div className="h-3" aria-hidden />
        </div>
      )}
    </div>
  );
}
