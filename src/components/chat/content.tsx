"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { ArrowDownIcon, InfoIcon, RotateCcwIcon } from "lucide-react";

import type { Locale } from "@/components/local/config";
import type { Event } from "../driver/types";
import { board as loadBoard, fill as fillFormula, formulas as loadFormulas, matchText, packInfo, stationLog } from "../pack/actions";
import type { Board, FormulaMeta, Match, PackInfo } from "../pack/types";
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
import { type Answered, DECLINED_MESSAGE, answersMessage, hideBlock, isAnswersMessage, parseAnswers, parseAsk } from "./ask-pure";
import type { Message } from "./message";
import { type Family, familyOf } from "./model-pure";
import { Composer, type Fill } from "./composer";
import { ProductionStrip } from "../production/content";
import { fillSlots, stageFormula } from "../production/pure";
import { startsFolded } from "./clamp-pure";
import { ContextLine, useWorkingContext } from "./context";
import { copy, greeting, tr } from "./config";
import { type Chip, type SlashEntry, chipFacts, matchSkills, packIds } from "./keywords";
import { type HomeOption, homeOptions, usableMatch } from "./options";
import { parseRefined } from "./refine";
import { Suggestions } from "./suggestions";
import { useDeptHealth } from "../pack/health";
import { AgentTurn, AnswerBlock, DeclinedLine, Prose, TurnActions, UserTurn } from "./turn";
import { Wizard } from "./wizard";
import { useStickToBottom } from "./use-stick-to-bottom";

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
  /** What the formula asked for (model, effort), spent by the next send. Settings → Model may pin the model over it. */
  const [formulaModel, setFormulaModel] = useState<string | null>(null);
  const [formulaEffort, setFormulaEffort] = useState<string | null>(null);
  const runModel = useRef<{ model: Family | null; effort: string | null }>({ model: null, effort: null });
  runModel.current = { model: familyOf(formulaModel), effort: formulaEffort };
  /** The film selected in the production strip: formulas on an existing film fill `project` with it. */
  const filmRef = useRef<string | null>(null);
  // The settings panel (the /social/publish gear): the film's stages, scenes and brand.
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [filmAvailable, setFilmAvailable] = useState(false)
  const toggleSettings = useCallback(() => setSettingsOpen((o) => !o), [])
  const onFilm = useCallback((id: string | null) => {
    filmRef.current = id;
  }, []);
  /** A video attached through + → Videos, until the next send: formulas with a `source` slot take its path. */
  const sourceRef = useRef<string | null>(null);
  /** The last fill: what it wrote, so an attached video can refill it while the box is untouched. */
  const lastFill = useRef<{ id: string; set: Record<string, string>; prompt: string } | null>(null);
  /** When the person tapped the formula the next send runs (ms), for the run's timing line. */
  const tapRef = useRef<number | null>(null);
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
            void speak(parseAsk(e.result).prose, isAr ? "ar" : "en").catch(() => {});
          patchLast((m) => {
            const failed = !e.ok && e.result !== "stopped";
            // A finished turn that ends with a `kun-questions` block shows the wizard under its prose.
            const { prose, questions } = failed ? { prose: m.text || e.result || "", questions: null } : parseAsk(m.text || e.result || "");
            return {
              ...settle(m),
              streaming: false,
              text: prose,
              asked: questions,
              error: failed ? t(copy.runError) : undefined,
            };
          });
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
    async (prompt: string, base: Message[] = messages, route?: string | null, extra?: Pick<Message, "answers" | "declined">) => {
      const text = prompt.trim();
      if (!text || busy) return;
      const timing = { formula: stationRef.current?.id ?? null, tTap: tapRef.current, tSend: Date.now() };
      tapRef.current = null;
      sourceRef.current = null;
      setFill(null);
      const { model, effort } = runModel.current;
      setFormulaModel(null);
      setFormulaEffort(null);
      const id = crypto.randomUUID();
      setJobId(id);
      const ranInPlan = routes.find((r) => r.id === route)?.mode === "plan";
      setMessages([
        ...base,
        { role: "user", text, tools: [], live: null, ...extra },
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
        await ask(text, { jobId: id, session, taskId: task ?? null, repo: ctx.repo || null, route: route ?? null, model, effort, timing }, onEvent);
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
    if (i >= 0) {
      const u = messages[i]!;
      void send(u.text, messages.slice(0, i), messages[i + 1]?.route ?? null, { answers: u.answers, declined: u.declined });
    }
  };

  const newChat = () => {
    if (busy) return;
    setMessages([]);
    setSession(null);
    setResumed(null);
    stationRef.current = null;
    sourceRef.current = null;
    lastFill.current = null;
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
  /** One fill through the station, with the selected film and the attached video handed over as slots; remembers it. */
  const fillWith = async (dept: string, id: string, set: Record<string, string> = {}) => {
    tapRef.current = Date.now();
    const slots = fillSlots(id, set, { project: filmRef.current, source: sourceRef.current });
    const f = await fillFormula(dept, id, ctx.repo || null, slots);
    lastFill.current = { id: f.id, set: slots, prompt: f.prompt };
    return f;
  };
  /** A tap: a formula option fills the box with the station's script (no model) and the person presses Send; a quick action writes its brief and refines. */
  const pickOption = async (o: HomeOption) => {
    if (o.logId) void stationLog(ctx.dept, ctx.repo || null, o.logId, "pick");
    if (o.formula) {
      try {
        const f = await fillWith(ctx.dept, o.formula, o.set);
        writeFilled(f.prompt, f.route ?? o.route, { id: f.id, record: f.record }, f);
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
      const f = await fillWith(dept, m.id, m.set ?? {});
      if (dept !== ctx.dept) carry.current = { dept, route: f.route };
      else if (f.route) setRouteId(f.route);
      stationRef.current = { id: f.id, record: f.record };
      setFormulaModel(f.model);
      setFormulaEffort(f.effort);
      void stationLog(dept, ctx.repo || null, f.id, "pick");
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
  /** Tab on the chip: the station's script fills the box (no model). */
  const acceptChip = (c: Extract<Chip, { kind: "formula" }>) => applyMatch(c.match);
  const dismissChip = (c: Chip) => {
    if (c.kind === "formula") void stationLog(ctx.dept, ctx.repo || null, c.id, "dismiss");
  };
  /** A formula chosen from the "/" menu: filled with no slots, so the open ones are for the person. */
  const pickFormula = async (id: string): Promise<string | null> => {
    try {
      const f = await fillWith(ctx.dept, id);
      if (f.route) setRouteId(f.route);
      stationRef.current = { id: f.id, record: f.record };
      setFormulaModel(f.model);
      setFormulaEffort(f.effort);
      void stationLog(ctx.dept, ctx.repo || null, id, "pick");
      return f.prompt;
    } catch {
      return null;
    }
  };
  /**
   * A row of the production strip: the formula fills the box through the same path as a station match (route, model,
   * record and the pick log). A formula this checkout does not ship (kun adds some later) falls to the visual stage.
   */
  const tapFormula = async (id: string, set: Record<string, string>) => {
    const known = formulaList.dept === ctx.dept && formulaList.list.some((f) => f.id === id);
    const use = known || formulaList.dept !== ctx.dept ? id : stageFormula("visual");
    const prompt = await applyMatch({ id: use, confidence: 1, dept: null, set });
    if (prompt !== null) setFill({ text: prompt, focus: true, n: ++fillN.current });
  };
  /**
   * A video attached through + → Videos is the film's source: later fills carry it, and a formula already in the box
   * (untouched) is filled again with it, which only changes the box when the formula has a `source` slot.
   */
  const attachVideo = async (path: string) => {
    sourceRef.current = path;
    const last = lastFill.current;
    if (!last || stationRef.current?.id !== last.id || draft.current !== last.prompt) return;
    try {
      const f = await fillFormula(ctx.dept, last.id, ctx.repo || null, { ...last.set, source: path });
      if (f.prompt === last.prompt) return;
      lastFill.current = { id: f.id, set: { ...last.set, source: path }, prompt: f.prompt };
      setFill({ text: f.prompt, focus: false, n: ++fillN.current });
    } catch {
      // the box keeps what it has; the next fill carries the source
    }
  };
  /**
   * Enter: in a new chat, a sentence the switch matches fills the box with the formula (the next Enter sends it);
   * anything else, a filled formula and any reply in a running chat send.
   */
  const submitText = async (text: string) => {
    if (!busy && messages.length === 0 && !stationRef.current) {
      const m = await matchText(ctx.dept, text, ctx.repo || null);
      if (usableMatch(m)) {
        const prompt = await applyMatch({ ...m, id: m.id });
        if (prompt !== null) {
          setFill({ text: prompt, focus: true, n: ++fillN.current });
          return;
        }
      }
    }
    sendRouted(text);
  };
  /** A held work order's Submit: clear the box, or put the caret in it to ask for the summary line. */
  const afterSubmit = () => setFill({ text: "", focus: false, n: ++fillN.current });
  const askSummary = () => setFill({ text: draft.current, focus: true, n: ++fillN.current });
  /** The title at the end of the wizard's bottom rule: the chat's own (resumed), else the opening line. */
  const firstUser = messages.find((m) => m.role === "user");
  const chatTitle = (resumed || (firstUser?.text.split("\n")[0] ?? "").trim().slice(0, 40) || t(copy.wizTitleFallback)).trim();
  /** The route the wizard's reply runs on: the turn that asked (the plan route), so the session carries on. */
  const asking = last?.route ?? chosen?.id ?? null;
  /** Submit: the answers go back into the same session at once. */
  const answerWizard = (items: Answered[]) => void send(answersMessage(items), undefined, asking, { answers: items });
  /** Cancel / Esc: the run is told to go on with sensible defaults. */
  const declineWizard = () => void send(DECLINED_MESSAGE, undefined, asking, { declined: true });
  /** "Chat about this": the wizard closes and the composer takes the focus; the reply goes as a normal message. */
  const chatAboutIt = () => {
    patchLast((m) => ({ ...m, asked: null }));
    setFill({ text: draft.current, focus: true, n: ++fillN.current });
  };
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

  const strip =
    ctx.dept === "video" ? (
      <ProductionStrip
        isAr={isAr}
        repo={ctx.repo}
        busy={busy}
        onPick={(id, set) => {
          setSettingsOpen(false)
          void tapFormula(id, set)
        }}
        onFilm={onFilm}
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        onAvailable={setFilmAvailable}
      />
    ) : null;

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
                <Composer
                  settings={ctx.dept === "video" ? { available: filmAvailable, open: settingsOpen, onToggle: toggleSettings, panel: strip } : undefined}
                  lang={lang}
                  onSend={(text) => void submitText(text)}
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
                  onAttachVideo={(p) => void attachVideo(p)}
                  autoFocus
                />
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
                if (m.role === "user" && m.declined) return <DeclinedLine key={i} text={t(copy.wizDeclined)} />;
                if (m.role === "user" && (m.answers?.length || isAnswersMessage(m.text)))
                  return <AnswerBlock key={i} lines={m.answers?.length ? m.answers : parseAnswers(m.text)} />;
                const note = splitNote(m.streaming ? hideBlock(m.text) : m.text);
                return m.role === "user" ? (
                  <UserTurn key={i} text={m.text} first={startsFolded(i)} isAr={isAr} />
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
                    {m.asked && i === messages.length - 1 && !busy && !m.streaming && (
                      <Wizard
                        key={`${i}.${m.text.length}`}
                        questions={m.asked}
                        isAr={isAr}
                        title={chatTitle}
                        onSubmit={answerWizard}
                        onCancel={declineWizard}
                        onChat={chatAboutIt}
                      />
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
                      asking: !!m.asked,
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
          <Composer
            settings={ctx.dept === "video" ? { available: filmAvailable, open: settingsOpen, onToggle: toggleSettings, panel: strip } : undefined}
            lang={lang}
            onSend={(text) => void submitText(text)}
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
            onAttachVideo={(p) => void attachVideo(p)}
            autoFocus
          />
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
