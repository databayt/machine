"use client";

import { Fragment, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  ArrowUpIcon,
  ClapperboardIcon,
  FileIcon,
  FileTextIcon,
  FolderIcon,
  ImageIcon,
  LinkIcon,
  LoaderIcon,
  MusicIcon,
  PlusIcon,
  SparklesIcon,
  SquareIcon,
  XIcon,
} from "lucide-react";

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { VoiceButton } from "../voice/content";
import { pickLocalFiles } from "../media/actions";
import type { PickKind } from "../media/kinds";
import {
  type Attachment,
  fromPath,
  mergeAttachments,
  withReferences,
} from "./attach";
import { copy, tr } from "./config";
import {
  type Chip,
  type SkillMatch,
  type SlashEntry,
  allSpans,
  firstOpenSlot,
  shouldProbe,
  slashContext,
  slashFilter,
} from "./keywords";
import { SlashMenu, slashOptionId } from "./slash";

/** A brief an option writes into the box; a new object (`n`) is a new fill. */
export interface Fill {
  text: string;
  /** Focus the box with the caret at the end (a leaf was chosen). */
  focus: boolean;
  /** Then enrich it: refine gathers the references and context into the brief. */
  refine?: boolean;
  n: number;
}

/** About six lines of text before the box scrolls. */
const MAX_HEIGHT_PX = 168;

/**
 * The chatgpt.com composer: one white pill with a soft shadow. `+` sits inside at the start (the attach
 * menu), the text grows to six lines, then the mic (dictation) and one dark round button at the end:
 * a send arrow (faded until there is text), or a stop square while a turn runs. Enter sends, Shift+Enter breaks a line.
 */
export function Composer({
  lang,
  onSend,
  onStop,
  busy,
  autoFocus,
  initialText,
  repo,
  fill,
  onRefine,
  onDraft,
  onProbe,
  onAcceptChip,
  onDismissChip,
  skillMatches,
  slashEntries,
  onPickFormula,
}: {
  lang: Locale;
  onSend: (text: string) => void;
  onStop: () => void;
  busy: boolean;
  autoFocus?: boolean;
  /** Fills the box without sending (a starter from the menu). */
  initialText?: string;
  /** The selected repository: its media library feeds the attach menu. */
  repo?: string;
  /** An option's brief: replaces the text, "" clears it. */
  fill?: Fill | null;
  /** Rewrites the text as a professional brief; null when it could not. */
  onRefine?: (text: string) => Promise<string | null>;
  /** Told the box's text on every change (the held task's Submit reads it as the summary). */
  onDraft?: (text: string) => void;
  /** After a pause on a draft of three words or more: the formula or skill the text names, or null. */
  onProbe?: (text: string) => Promise<Chip | null>;
  /** Tab on a formula chip: fills it (the station's script) and returns the prompt, or null. */
  onAcceptChip?: (chip: Extract<Chip, { kind: "formula" }>) => Promise<string | null>;
  /** Esc on a formula chip: the station logs the dismissal. */
  onDismissChip?: (chip: Chip) => void;
  /** The skills the text names (underlined inline); pure and cheap, it runs on every change. */
  skillMatches?: (text: string) => SkillMatch[];
  /** The "/" menu's rows: formulas, the pack's skills, the other skills. */
  slashEntries?: SlashEntry[];
  /** A formula picked from the "/" menu: filled with no slots; the prompt, or null. */
  onPickFormula?: (id: string) => Promise<string | null>;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const [text, setText] = useState(initialText ?? "");
  const [plus, setPlus] = useState(false);
  /** `+` is one flat attach menu; "Link" turns its row into an input in place. */
  const [attached, setAttached] = useState<Attachment[]>([]);
  const ref = useRef<HTMLTextAreaElement>(null);
  const [refining, setRefining] = useState(false);
  const [refineError, setRefineError] = useState(false);
  /** The text before the last refine, and what refine wrote: ⌘Z swaps back while it is untouched. */
  const beforeRefine = useRef<{ before: string; after: string } | null>(null);
  const [caret, setCaret] = useState(0);
  const [chip, setChip] = useState<Chip | null>(null);
  /** A formula the person dismissed with Esc stays quiet until the box empties or is sent. */
  const dismissed = useRef<string | null>(null);
  const probeSeq = useRef(0);
  /** Text the composer wrote itself (a fill, refine, Tab): never probed again. */
  const skipProbe = useRef<string | null>(null);
  const probeRef = useRef(onProbe);
  probeRef.current = onProbe;
  const [slashIdx, setSlashIdx] = useState(0);
  const [slashOff, setSlashOff] = useState<string | null>(null);
  const [overflowing, setOverflowing] = useState(false);
  const listId = useId();
  /** Sets text the composer wrote itself. */
  const setWritten = (v: string) => {
    skipProbe.current = v;
    setText(v);
  };

  useEffect(() => {
    onDraft?.(text);
  }, [text, onDraft]);

  // `field-sizing: content` is not in Safari yet.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, MAX_HEIGHT_PX)}px`;
    setOverflowing(el.scrollHeight > MAX_HEIGHT_PX);
  }, [text]);

  // After a pause the station reads the draft; a stale answer is dropped. Text the composer wrote
  // itself, a slash command and short drafts are never probed.
  const probing = !!onProbe;
  useEffect(() => {
    const seq = ++probeSeq.current;
    if (!probing || text === skipProbe.current || !shouldProbe(text)) {
      setChip(null);
      return;
    }
    const h = setTimeout(() => {
      void probeRef
        .current?.(text)
        .catch(() => null)
        .then((c) => {
          if (seq !== probeSeq.current) return;
          setChip(c && !(c.kind === "formula" && c.id === dismissed.current) ? c : null);
        });
    }, 250);
    return () => clearTimeout(h);
  }, [text, probing]);
  useEffect(() => {
    if (!text.trim()) dismissed.current = null;
  }, [text]);

  const slash = useMemo(() => slashContext(text, caret), [text, caret]);
  const slashKey = slash ? `${slash.start}:${slash.fragment}` : null;
  const slashItems = useMemo(
    () => (slash && slashEntries ? slashFilter(slash.fragment, slashEntries) : []),
    [slash, slashEntries],
  );
  const slashOpen = slashItems.length > 0 && slashKey !== slashOff;
  useEffect(() => setSlashIdx(0), [slashKey]);
  const slashAt = Math.min(slashIdx, Math.max(slashItems.length - 1, 0));
  const chipShown = chip && chip.forText === text && !slashOpen && !busy ? chip : null;
  const spans = useMemo(
    () => (skillMatches && !overflowing && text ? allSpans(skillMatches(text)) : []),
    [skillMatches, text, overflowing],
  );

  /** Caret at the first open slot of a filled prompt (selected, so typing replaces it), else at the end. */
  const placeCaret = (out: string) =>
    requestAnimationFrame(() => {
      const el = ref.current;
      if (!el) return;
      el.focus();
      const slot = firstOpenSlot(out);
      el.setSelectionRange(slot ? slot[0] : out.length, slot ? slot[1] : out.length);
      setCaret(el.selectionStart);
    });
  /** A filled prompt replaces the draft; ⌘Z brings the draft back. */
  const writeFilled = (out: string) => {
    beforeRefine.current = { before: text, after: out };
    setWritten(out);
    placeCaret(out);
  };
  const acceptChip = async (c: Chip) => {
    if (c.kind === "skill") {
      const out = `/${c.name} ${text}`;
      beforeRefine.current = { before: text, after: out };
      setWritten(out);
      placeCaret(out);
      return;
    }
    const out = await onAcceptChip?.(c).catch(() => null);
    if (out) writeFilled(out);
  };
  const dismissChip = (c: Chip) => {
    if (c.kind === "formula") dismissed.current = c.id;
    setChip(null);
    onDismissChip?.(c);
  };
  const pickSlash = async (e: SlashEntry) => {
    if (!slash) return;
    if (e.kind === "formula") {
      const out = await onPickFormula?.(e.id).catch(() => null);
      if (out) writeFilled(out);
      else setSlashOff(slashKey);
      return;
    }
    const ins = `/${e.name} `;
    const out = text.slice(0, slash.start) + ins + text.slice(caret);
    const at = slash.start + ins.length;
    setWritten(out);
    requestAnimationFrame(() => {
      const el = ref.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(at, at);
      setCaret(at);
    });
  };

  // A starter fills the box; the caret goes to its end so the person carries on typing.
  useEffect(() => {
    const el = ref.current;
    if (el && initialText)
      el.setSelectionRange(el.value.length, el.value.length);
  }, [initialText]);

  // An option's brief replaces the box; the caret goes to its end so the person edits on.
  useEffect(() => {
    if (!fill) return;
    beforeRefine.current = null;
    setWritten(fill.text);
    if (fill.text && fill.refine) void refineRef.current(fill.text);
    if (fill.focus) {
      requestAnimationFrame(() => {
        const el = ref.current;
        if (!el) return;
        el.focus();
        el.setSelectionRange(el.value.length, el.value.length);
      });
    }
  }, [fill]);

  const refine = async (input: string = text) => {
    if (!onRefine || refining || !input.trim()) return;
    const before = input;
    setRefining(true);
    setRefineError(false);
    const out = await onRefine(before).catch(() => null);
    setRefining(false);
    if (!out) {
      setRefineError(true);
      return;
    }
    beforeRefine.current = { before, after: out };
    setWritten(out);
    requestAnimationFrame(() => {
      const el = ref.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    });
  };

  const refineRef = useRef(refine);
  refineRef.current = refine;
  const hasText = !!text.trim();
  const submit = () => {
    if (busy || !hasText) return;
    onSend(withReferences(text, attached));
    dismissed.current = null;
    setText("");
    beforeRefine.current = null;
    setAttached([]);
  };
  const add = (more: Attachment[]) =>
    setAttached((have) => mergeAttachments(have, more));
  const addFiles = async (kind: PickKind) => {
    setPlus(false);
    const paths = await pickLocalFiles(t(copy.pickFiles), kind).catch(() => []);
    if (paths.length) add(paths.map(fromPath));
  };
  const pickers: { kind: PickKind; label: keyof typeof copy; Icon: typeof FileIcon }[] = [
    { kind: "image", label: "attachImages", Icon: ImageIcon },
    { kind: "video", label: "attachVideos", Icon: ClapperboardIcon },
    { kind: "audio", label: "attachAudio", Icon: MusicIcon },
    { kind: "document", label: "attachDocs", Icon: FileTextIcon },
  ];
  const row =
    "flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-start text-[15px] transition-colors hover:bg-muted disabled:opacity-50";
  const icon =
    "flex size-10 shrink-0 items-center justify-center rounded-full transition-colors";

  return (
    <>
      <div className="relative">
      {slashOpen && (
        <SlashMenu
          entries={slashItems}
          index={slashAt}
          isAr={isAr}
          listId={listId}
          onPick={(e) => void pickSlash(e)}
          onHover={setSlashIdx}
        />
      )}
      {chipShown && (
        <button
          type="button"
          tabIndex={-1}
          onMouseDown={(e) => {
            e.preventDefault();
            void acceptChip(chipShown);
          }}
          aria-label={t(copy.chipLabel)}
          dir={isAr ? "rtl" : "ltr"}
          className="absolute bottom-full start-5 mb-1.5 flex max-w-[calc(100%-2.5rem)] items-center gap-1.5 text-xs text-muted-foreground/80"
        >
          <span aria-hidden>⇥</span>
          <span>{t(copy.chipTab)}</span>
          <span aria-hidden>·</span>
          <span className="truncate" dir="auto">
            {chipShown.kind === "skill"
              ? `/${chipShown.name}`
              : [chipShown.label, ...chipShown.facts].join(" · ")}
          </span>
        </button>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="flex min-h-[60px] w-full flex-col rounded-[30px] border border-border/60 bg-background p-2 shadow-[0_4px_16px_-10px_rgb(0_0_0/0.10)] transition-shadow focus-within:shadow-[0_4px_18px_-10px_rgb(0_0_0/0.14)]"
      >
        {attached.length > 0 && (
          <ul
            dir={isAr ? "rtl" : "ltr"}
            className="flex flex-wrap gap-2 px-2 pb-1 pt-1"
          >
            {attached.map((a) => (
              <li
                key={a.key}
                className="flex max-w-48 items-center gap-2 rounded-xl bg-muted/60 p-1 pe-1.5"
              >
                <span className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted text-muted-foreground">
                  {a.thumb ? (
                    // eslint-disable-next-line @next/next/no-img-element -- CDN url in a static export
                    <img
                      src={a.thumb}
                      alt=""
                      loading="lazy"
                      className="size-full object-cover"
                    />
                  ) : a.kind === "video" ? (
                    <ClapperboardIcon className="size-5" aria-hidden />
                  ) : a.kind === "image" ? (
                    <ImageIcon className="size-5" aria-hidden />
                  ) : a.kind === "audio" ? (
                    <MusicIcon className="size-5" aria-hidden />
                  ) : a.kind === "folder" ? (
                    <FolderIcon className="size-5" aria-hidden />
                  ) : a.kind === "link" ? (
                    <LinkIcon className="size-5" aria-hidden />
                  ) : (
                    <FileIcon className="size-5" aria-hidden />
                  )}
                </span>
                <span className="min-w-0 flex-1 truncate text-xs" dir="auto">
                  {a.title}
                </span>
                <button
                  type="button"
                  onClick={() =>
                    setAttached((all) => all.filter((x) => x.key !== a.key))
                  }
                  aria-label={`${t(copy.remove)}: ${a.title}`}
                  className="flex size-5 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
                >
                  <XIcon className="size-3.5" aria-hidden />
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="flex w-full items-end gap-1">
          <Popover
            open={plus}
            onOpenChange={setPlus}
          >
            <PopoverTrigger asChild>
              <button
                type="button"
                aria-label={t(copy.attach)}
                title={t(copy.attach)}
                className={cn(
                  icon,
                  "text-foreground hover:bg-muted",
                  plus && "bg-muted",
                )}
              >
                <PlusIcon
                  className={cn(
                    "size-6 transition-transform",
                    plus && "rotate-45",
                  )}
                  strokeWidth={1.5}
                  aria-hidden
                />
              </button>
            </PopoverTrigger>
            <PopoverContent
              side="bottom"
              align="start"
              sideOffset={14}
              collisionPadding={16}
              dir={isAr ? "rtl" : "ltr"}
              className="w-64 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5"
            >
              {pickers.map(({ kind, label, Icon }) => (
                <button
                  key={kind}
                  type="button"
                  onClick={() => void addFiles(kind)}
                  className={row}
                >
                  <Icon className="size-4 text-muted-foreground" aria-hidden />
                  {t(copy[label])}
                </button>
              ))}
            </PopoverContent>
          </Popover>

          <div className="relative min-w-0 flex-1 self-center">
            {spans.length > 0 && (
              <div
                aria-hidden
                dir="auto"
                className="pointer-events-none absolute inset-0 overflow-hidden whitespace-pre-wrap break-words px-1 py-2 text-[17px] leading-6 text-transparent"
              >
                {highlight(text, spans)}
              </div>
            )}
            <textarea
              ref={ref}
              value={text}
              onChange={(e) => {
                setText(e.target.value);
                setCaret(e.target.selectionStart);
                setRefineError(false);
              }}
              onSelect={(e) => setCaret(e.currentTarget.selectionStart)}
              onKeyDown={(e) => {
                const undo = beforeRefine.current;
                if (e.key === "z" && (e.metaKey || e.ctrlKey) && !e.shiftKey && undo && text === undo.after) {
                  e.preventDefault();
                  beforeRefine.current = null;
                  setWritten(undo.before);
                  return;
                }
                // Mid-composition (IME) every key belongs to the input method.
                if (e.nativeEvent.isComposing || e.keyCode === 229) return;
                if (slashOpen) {
                  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                    e.preventDefault();
                    const n = slashItems.length;
                    setSlashIdx((slashAt + (e.key === "ArrowDown" ? 1 : n - 1)) % n);
                    return;
                  }
                  if (e.key === "Enter" || (e.key === "Tab" && !e.shiftKey)) {
                    e.preventDefault();
                    void pickSlash(slashItems[slashAt]!);
                    return;
                  }
                  if (e.key === "Escape") {
                    e.preventDefault();
                    e.stopPropagation();
                    setSlashOff(slashKey);
                    return;
                  }
                }
                if (chipShown && e.key === "Tab" && !e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey) {
                  e.preventDefault();
                  void acceptChip(chipShown);
                  return;
                }
                if (chipShown && e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  dismissChip(chipShown);
                  return;
                }
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  submit();
                }
              }}
              rows={1}
              maxLength={4000}
              autoFocus={autoFocus}
              enterKeyHint="send"
              dir="auto"
              role="combobox"
              aria-expanded={slashOpen}
              aria-controls={slashOpen ? listId : undefined}
              aria-activedescendant={slashOpen ? slashOptionId(listId, slashAt) : undefined}
              aria-autocomplete="list"
              placeholder={t(copy.placeholder)}
              aria-label={t(copy.placeholder)}
              className="relative block w-full resize-none bg-transparent px-1 py-2 text-[17px] leading-6 outline-none placeholder:text-muted-foreground"
            />
          </div>

          {!busy && onRefine && hasText && (
            <button
              type="button"
              onClick={() => void refine()}
              disabled={refining}
              aria-label={t(refining ? copy.refining : refineError ? copy.refineFailed : copy.refine)}
              title={t(refining ? copy.refining : refineError ? copy.refineFailed : copy.refine)}
              className={cn(icon, "text-foreground hover:bg-muted", refineError && "text-destructive")}
            >
              {refining ? (
                <LoaderIcon className="size-5 animate-spin" strokeWidth={1.5} aria-hidden />
              ) : (
                <SparklesIcon className="size-5" strokeWidth={1.5} aria-hidden />
              )}
            </button>
          )}

          {!busy && (
            <span className="contents">
              <VoiceButton
                lang={lang}
                onTranscript={(v) =>
                  setText((prev) => (prev.trim() ? `${prev.trim()} ${v}` : v))
                }
              />
            </span>
          )}

          {busy ? (
            <button
              type="button"
              onClick={onStop}
              aria-label={t(copy.stop)}
              title={t(copy.stop)}
              className={cn(icon, "bg-foreground text-background")}
            >
              <SquareIcon className="size-3.5 fill-current" aria-hidden />
            </button>
          ) : (
            <button
              type="submit"
              aria-label={t(copy.send)}
              title={t(copy.send)}
              disabled={!hasText}
              className={cn(
                icon,
                "bg-foreground text-background hover:bg-foreground/85 disabled:bg-foreground/15 disabled:text-background",
              )}
            >
              <ArrowUpIcon className="size-5" strokeWidth={2.5} aria-hidden />
            </button>
          )}
        </div>
      </form>
      </div>
    </>
  );
}

/** The text with each span underlined, the rest unchanged: it mirrors the textarea under it. */
function highlight(text: string, spans: [number, number][]) {
  const out: React.ReactNode[] = [];
  let at = 0;
  spans.forEach(([a, b], i) => {
    if (a > at) out.push(text.slice(at, a));
    out.push(
      <span key={i} className="underline decoration-foreground/30 decoration-1 underline-offset-4">
        {text.slice(a, b)}
      </span>,
    );
    at = b;
  });
  out.push(text.slice(at));
  return <Fragment>{out}</Fragment>;
}
