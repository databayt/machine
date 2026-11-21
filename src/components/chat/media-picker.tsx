"use client";

import { useEffect, useMemo, useState } from "react";
import { CheckIcon, ClapperboardIcon, SearchIcon } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import type { Locale } from "@/components/local/config";
import { mediaLibrary } from "../media/actions";
import type { MediaItem } from "../media/types";
import { itemTitle } from "./attach";
import { copy, tr } from "./config";

type Kind = "all" | "image" | "video";
type Lang = "all" | "ar" | "en";

const clock = (s: number) =>
  `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

/**
 * The media library of the selected repository as a picker: a grid of thumbnails straight from the CDN,
 * a search box (title in both languages, id), kind tabs and a language filter. Multi-select, then Attach.
 */
export function MediaPicker({
  lang,
  repo,
  open,
  onOpenChange,
  onAttach,
}: {
  lang: Locale;
  repo: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAttach: (items: MediaItem[]) => void;
}) {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const [items, setItems] = useState<MediaItem[] | null>(null);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<Kind>("all");
  const [only, setOnly] = useState<Lang>("all");
  const [picked, setPicked] = useState<Set<string>>(new Set());

  // Opening starts a fresh choice; the library is read again (the shell caches it for ten minutes).
  useEffect(() => {
    if (!open) return;
    setPicked(new Set());
    setQuery("");
    setItems(null);
    let live = true;
    mediaLibrary(repo)
      .then((all) => live && setItems(all))
      .catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [open, repo]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (items ?? []).filter(
      (i) =>
        (kind === "all" || i.kind === kind) &&
        (only === "all" || i.lang === only || i.lang === "") &&
        (!q ||
          `${i.id} ${i.title_ar} ${i.title_en}`.toLowerCase().includes(q)),
    );
  }, [items, query, kind, only]);

  const toggle = (id: string) =>
    setPicked((p) => {
      const next = new Set(p);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const attach = () => {
    onAttach((items ?? []).filter((i) => picked.has(i.id)));
    onOpenChange(false);
  };

  const chip =
    "rounded-full px-3 py-1 text-sm transition-colors hover:bg-muted data-[on=true]:bg-foreground data-[on=true]:text-background";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        dir={isAr ? "rtl" : "ltr"}
        className="flex h-[min(80vh,640px)] flex-col gap-3 sm:max-w-3xl"
      >
        <DialogTitle>{t(copy.library)}</DialogTitle>
        <DialogDescription className="sr-only">
          {t(copy.libraryHint)}
        </DialogDescription>

        <div className="relative">
          <SearchIcon
            className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t(copy.search)}
            aria-label={t(copy.search)}
            className="ps-9"
          />
        </div>

        <div className="flex flex-wrap items-center justify-between gap-2">
          <Tabs value={kind} onValueChange={(v) => setKind(v as Kind)}>
            <TabsList>
              <TabsTrigger value="all">{t(copy.all)}</TabsTrigger>
              <TabsTrigger value="image">{t(copy.images)}</TabsTrigger>
              <TabsTrigger value="video">{t(copy.videos)}</TabsTrigger>
            </TabsList>
          </Tabs>
          <div className="flex items-center gap-1">
            {(
              [
                ["all", copy.all],
                ["ar", copy.langAr],
                ["en", copy.langEn],
              ] as const
            ).map(([v, label]) => (
              <button
                key={v}
                type="button"
                data-on={only === v}
                onClick={() => setOnly(v)}
                className={chip}
              >
                {t(label)}
              </button>
            ))}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
          {items === null ? (
            <p className="py-10 text-center text-sm text-muted-foreground">
              {t(copy.loading)}
            </p>
          ) : shown.length === 0 ? (
            <p className="py-10 text-center text-sm text-muted-foreground">
              {t(items.length === 0 ? copy.noMedia : copy.noMatch)}
            </p>
          ) : (
            <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4">
              {shown.map((i) => {
                const on = picked.has(i.id);
                const title = itemTitle(i, isAr);
                return (
                  <li key={i.id}>
                    <button
                      type="button"
                      onClick={() => toggle(i.id)}
                      aria-pressed={on}
                      title={title}
                      className="group block w-full text-start"
                    >
                      <span
                        className={cn(
                          "relative block aspect-video overflow-hidden rounded-xl bg-muted ring-2 ring-transparent transition",
                          on && "ring-foreground",
                        )}
                      >
                        {i.thumb ? (
                          // eslint-disable-next-line @next/next/no-img-element -- CDN url in a static export
                          <img
                            src={i.thumb}
                            alt=""
                            loading="lazy"
                            decoding="async"
                            className="size-full object-cover"
                          />
                        ) : (
                          <span className="flex size-full items-center justify-center text-muted-foreground">
                            <ClapperboardIcon className="size-6" aria-hidden />
                          </span>
                        )}
                        {i.kind === "video" && i.duration > 0 && (
                          <span className="absolute bottom-1 end-1 rounded bg-black/60 px-1.5 text-[11px] text-white" dir="ltr">
                            {clock(i.duration)}
                          </span>
                        )}
                        {on && (
                          <span className="absolute start-1.5 top-1.5 flex size-5 items-center justify-center rounded-full bg-foreground text-background">
                            <CheckIcon className="size-3.5" aria-hidden />
                          </span>
                        )}
                      </span>
                      <span className="mt-1 block truncate text-xs text-muted-foreground group-hover:text-foreground">
                        {title}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        <div className="flex justify-end">
          <button
            type="button"
            disabled={picked.size === 0}
            onClick={attach}
            className="rounded-full bg-foreground px-5 py-2 text-sm font-medium text-background transition-colors hover:bg-foreground/85 disabled:bg-foreground/15"
          >
            {t(copy.attachN)}
            {picked.size > 0 ? ` (${picked.size})` : ""}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
