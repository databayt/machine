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
  PlusIcon,
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

/** A sentence the navigator writes into the box; a new object (`n`) is a new fill. */
export interface Fill {
  text: string;
  /** Focus the box with the caret at the end (a leaf was chosen). */
  focus: boolean;
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
}: {
  lang: Locale;
  /** `mode: "plan"` when the text came from the navigator and the chip is still on. */
  onSend: (text: string, mode?: "plan") => void;
  onStop: () => void;
  busy: boolean;
  actions: QuickAction[];
  autoFocus?: boolean;
  /** Fills the box without sending (a starter from the menu). */
  initialText?: string;
  /** The selected repository: its media library feeds the attach menu. */
  repo?: string;
  /** The navigator's sentence: replaces the text (autocomplete), "" clears an untouched one. */
  fill?: Fill | null;
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
  /** The text came from the navigator: it goes out as a plan turn until the chip's x or an empty box. */
  const [plan, setPlan] = useState(false);
  const lastFill = useRef("");
  const textRef = useRef(text);
  textRef.current = text;
  const planRef = useRef(plan);
  planRef.current = plan;

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

  // The navigator's sentence replaces the box, live at every step. An empty one (back at the
  // root) only clears text that is still the navigator's: the chip is on, or it is untouched.
  useEffect(() => {
    if (!fill) return;
    if (fill.text) {
      lastFill.current = fill.text;
      setText(fill.text);
      setPlan(true);
      if (fill.focus) {
        requestAnimationFrame(() => {
          const el = ref.current;
          if (!el) return;
          el.focus();
          el.setSelectionRange(el.value.length, el.value.length);
        });
      }
    } else if (planRef.current || textRef.current === lastFill.current) {
      lastFill.current = "";
      setText("");
      setPlan(false);
    }
  }, [fill]);

  const hasText = !!text.trim();
  const submit = () => {
    if (busy || !hasText) return;
    onSend(withReferences(text, attached), plan ? "plan" : undefined);
    setText("");
    setPlan(false);
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
          {plan && (
            <span className="flex h-10 shrink-0 items-center">
              <span className="flex items-center gap-1 rounded-full bg-muted/60 py-0.5 ps-2.5 pe-1 text-xs text-muted-foreground">
                {t(copy.planFirst)}
                <button
                  type="button"
                  onClick={() => setPlan(false)}
                  aria-label={t(copy.planOff)}
                  title={t(copy.planOff)}
                  className="flex size-5 items-center justify-center rounded-full transition-colors hover:bg-foreground/10 hover:text-foreground"
                >
                  <XIcon className="size-3" aria-hidden />
                </button>
              </span>
            </span>
          )}
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
                                onSend(q.prompt);
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
              if (!e.target.value.trim()) setPlan(false);
            }}
            onKeyDown={(e) => {
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
