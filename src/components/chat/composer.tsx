"use client";

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  ArrowLeftIcon,
  ArrowUpIcon,
  ChevronRightIcon,
  ClapperboardIcon,
  FileIcon,
  ImageIcon,
  MonitorIcon,
  PaperclipIcon,
  LoaderIcon,
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
import type { QuickAction } from "../pack/types";
import { VoiceButton } from "../voice/content";
import { pickLocalFiles } from "../media/actions";
import {
  type Attachment,
  fromItem,
  fromPath,
  mergeAttachments,
  withReferences,
} from "./attach";
import { copy, tr } from "./config";
import { MediaPicker } from "./media-picker";

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
 * The chatgpt.com composer: one white pill with a soft shadow. `+` sits inside at the start (the quick
 * actions), the text grows to six lines, then the mic (dictation) and one dark round button at the end:
 * a send arrow (faded until there is text), or a stop square while a turn runs. Enter sends, Shift+Enter breaks a line.
 */
export function Composer({
  lang,
  onSend,
  onStop,
  busy,
  actions,
  autoFocus,
  initialText,
  repo,
  fill,
  onPickAction,
  onRefine,
}: {
  lang: Locale;
  onSend: (text: string) => void;
  onStop: () => void;
  busy: boolean;
  actions: QuickAction[];
  autoFocus?: boolean;
  /** Fills the box without sending (a starter from the menu). */
  initialText?: string;
  /** The selected repository: its media library feeds the attach menu. */
  repo?: string;
  /** An option's brief: replaces the text, "" clears it. */
  fill?: Fill | null;
  /** A quick action from `+`: the parent fills its brief and its route. */
  onPickAction?: (q: QuickAction) => void;
  /** Rewrites the text as a professional brief; null when it could not. */
  onRefine?: (text: string) => Promise<string | null>;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const [text, setText] = useState(initialText ?? "");
  const [plus, setPlus] = useState(false);
  /** The menu is two levels in one popover: the quick actions, and "Attach" (a panel swapped in place). */
  const [view, setView] = useState<"root" | "attach">("root");
  const [library, setLibrary] = useState(false);
  const [attached, setAttached] = useState<Attachment[]>([]);
  const ref = useRef<HTMLTextAreaElement>(null);
  const [refining, setRefining] = useState(false);
  const [refineError, setRefineError] = useState(false);
  /** The text before the last refine, and what refine wrote: ⌘Z swaps back while it is untouched. */
  const beforeRefine = useRef<{ before: string; after: string } | null>(null);

  // `field-sizing: content` is not in Safari yet.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, MAX_HEIGHT_PX)}px`;
  }, [text]);

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
    setText(fill.text);
    if (fill.text && fill.refine) void refineRef.current(fill.text);
    if (fill.text) {
      if (fill.focus) {
        requestAnimationFrame(() => {
          const el = ref.current;
          if (!el) return;
          el.focus();
          el.setSelectionRange(el.value.length, el.value.length);
        });
      }
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
    setText(out);
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
    setText("");
    beforeRefine.current = null;
    setAttached([]);
  };
  const addFiles = async () => {
    setPlus(false);
    const paths = await pickLocalFiles(t(copy.pickFiles)).catch(() => []);
    if (paths.length)
      setAttached((have) => mergeAttachments(have, paths.map(fromPath)));
  };
  const row =
    "flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-start text-[15px] transition-colors hover:bg-muted disabled:opacity-50";
  const icon =
    "flex size-10 shrink-0 items-center justify-center rounded-full transition-colors";

  return (
    <>
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
            onOpenChange={(o) => {
              setPlus(o);
              if (o) setView("root");
            }}
          >
            <PopoverTrigger asChild>
              <button
                type="button"
                aria-label={t(copy.quick)}
                title={t(copy.quick)}
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
              collisionPadding={16}
              dir={isAr ? "rtl" : "ltr"}
              className="w-72 max-w-[calc(100vw-2rem)] rounded-2xl p-1.5"
            >
              {view === "root" ? (
                <>
                  {actions.length > 0 && (
                    <>
                      <p className="px-3 pb-1 pt-2 text-sm font-semibold">
                        {t(copy.quick)}
                      </p>
                      <ul>
                        {actions.map((q) => (
                          <li key={q.prompt}>
                            <button
                              type="button"
                              disabled={busy}
                              onClick={() => {
                                setPlus(false);
                                if (onPickAction) onPickAction(q);
                                else onSend(q.prompt);
                              }}
                              className={row}
                            >
                              {isAr ? q.label_ar : q.label_en}
                            </button>
                          </li>
                        ))}
                      </ul>
                      <div
                        className="mx-2 my-1 h-px bg-border/60"
                        aria-hidden
                      />
                    </>
                  )}
                  <button
                    type="button"
                    onClick={() => setView("attach")}
                    className={row}
                  >
                    <PaperclipIcon
                      className="size-4 text-muted-foreground"
                      aria-hidden
                    />
                    <span className="flex-1">{t(copy.attach)}</span>
                    <ChevronRightIcon
                      className="size-4 text-muted-foreground rtl:rotate-180"
                      aria-hidden
                    />
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    onClick={() => setView("root")}
                    className="flex w-full items-center gap-2 rounded-xl px-3 pb-1 pt-2 text-start text-sm font-semibold transition-colors hover:bg-muted"
                  >
                    <ArrowLeftIcon
                      className="size-4 rtl:rotate-180"
                      aria-hidden
                    />
                    {t(copy.attach)}
                  </button>
                  <button
                    type="button"
                    disabled={!repo}
                    onClick={() => {
                      setPlus(false);
                      setLibrary(true);
                    }}
                    className={row}
                  >
                    <ImageIcon
                      className="size-4 text-muted-foreground"
                      aria-hidden
                    />
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span>{t(copy.fromLibrary)}</span>
                      {!repo && (
                        <span className="text-xs text-muted-foreground">
                          {t(copy.pickRepo)}
                        </span>
                      )}
                    </span>
                  </button>
                  <button
                    type="button"
                    onClick={() => void addFiles()}
                    className={row}
                  >
                    <MonitorIcon
                      className="size-4 text-muted-foreground"
                      aria-hidden
                    />
                    {t(copy.fromMac)}
                  </button>
                </>
              )}
            </PopoverContent>
          </Popover>

          <textarea
            ref={ref}
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setRefineError(false);
            }}
            onKeyDown={(e) => {
              const undo = beforeRefine.current;
              if (e.key === "z" && (e.metaKey || e.ctrlKey) && !e.shiftKey && undo && text === undo.after) {
                e.preventDefault();
                beforeRefine.current = null;
                setText(undo.before);
                return;
              }
              if (
                e.key === "Enter" &&
                !e.shiftKey &&
                !e.nativeEvent.isComposing
              ) {
                e.preventDefault();
                submit();
              }
            }}
            rows={1}
            maxLength={4000}
            autoFocus={autoFocus}
            enterKeyHint="send"
            dir="auto"
            placeholder={t(copy.placeholder)}
            aria-label={t(copy.placeholder)}
            className="min-w-0 flex-1 resize-none self-center bg-transparent px-1 py-2 text-[17px] leading-6 outline-none placeholder:text-muted-foreground"
          />

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
      {repo && (
        <MediaPicker
          lang={lang}
          repo={repo}
          open={library}
          onOpenChange={setLibrary}
          onAttach={(items) =>
            setAttached((have) =>
              mergeAttachments(
                have,
                items.map((i) => fromItem(i, isAr)),
              ),
            )
          }
        />
      )}
    </>
  );
}
