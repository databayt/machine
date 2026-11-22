"use client";

import { useState } from "react";

import type { Locale } from "@/components/local/config";
import { pickLocalFiles } from "../media/actions";
import { submitWork } from "./actions";
import { copy } from "./config";
import { progressLine, splitSummary } from "./submit";

/**
 * The one button of a held work order: Submit («سلّم العمل»). It reads the box as the summary (and a
 * `next:` line as the line for the next person), opens the file picker, uploads the files with a progress
 * line, and posts the receipt. With nothing in the box it asks for one line instead. A refusal shows
 * kun's own reason in place of the progress line.
 */
export function SubmitWork({
  lang,
  task,
  draft,
  onDone,
  onAskSummary,
}: {
  lang: Locale;
  /** The held task, `databayt/<repo>#<n>`. */
  task: string;
  /** The composer's current text. */
  draft: () => string;
  /** The files are in and the receipt posted: clear the box. */
  onDone: () => void;
  /** Nothing in the box: focus it. */
  onAskSummary: () => void;
}) {
  const isAr = lang === "ar";
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en);
  const [busy, setBusy] = useState(false);
  const [line, setLine] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  async function submit() {
    const { summary, next } = splitSummary(draft());
    if (!summary) {
      setFailed(false);
      setLine(t(copy.submitAsk));
      onAskSummary();
      return;
    }
    const paths = await pickLocalFiles(t(copy.submitPick), "any").catch(() => []);
    if (paths.length === 0) return;
    setBusy(true);
    setFailed(false);
    setLine(null);
    try {
      await submitWork(task, paths, summary, next, (e) => setLine(progressLine(e, isAr)));
      setLine(t(copy.submitDone));
      onDone();
    } catch (e) {
      setFailed(true);
      setLine(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 py-3">
      <button
        type="button"
        disabled={busy}
        onClick={() => void submit()}
        className="inline-flex shrink-0 items-center rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background transition-colors hover:bg-foreground/85 disabled:opacity-50"
      >
        {t(copy.submit)}
      </button>
      {line && (
        <p
          role={failed ? "alert" : "status"}
          dir="auto"
          className={`min-w-0 text-sm ${failed ? "text-destructive" : "text-muted-foreground"}`}
        >
          {line}
        </p>
      )}
    </div>
  );
}
