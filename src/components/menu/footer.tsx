"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";

import type { Locale } from "@/components/local/config";
import type { HealthReport } from "./data";
import { tr } from "../chat/config";
import { listSessions } from "../sessions/actions";
import type { Session } from "../sessions/types";
import { storeLang, swapLocale } from "../shell/lang";
import { useNav } from "../shell/use-nav";
import { copy, NEW_CHAT_EVENT } from "./config";
import { Dot } from "./ui";

const item = "shrink-0 whitespace-nowrap transition-colors hover:text-foreground";

/** The footer strip: small gray, one row, centred. */
export function Footer({
  lang,
  inApp,
  health,
}: {
  lang: Locale;
  inApp: boolean;
  /** Kept for the menu's call; the footer reads the same tree as the sidebar, which gates the engine itself. */
  showEngine?: boolean;
  health: HealthReport | null;
}) {
  const t = tr(lang === "ar");
  const { tree, trail } = useNav(lang);
  const router = useRouter();
  const pathname = usePathname();
  const [chats, setChats] = useState<Session[] | null>(null);
  const [panel, setPanel] = useState(false);

  // Recent chats: five, newest first, loaded when the popover opens.
  useEffect(() => {
    if (!panel) return;
    listSessions()
      .then((rows) => setChats([...rows].sort((a, b) => b.updated_at - a.updated_at).slice(0, 5)))
      .catch(() => setChats([]));
    const away = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest("[data-chats]")) setPanel(false);
    };
    document.addEventListener("click", away);
    return () => document.removeEventListener("click", away);
  }, [panel]);

  const other: Locale = lang === "ar" ? "en" : "ar";
  const bad = health?.items.filter((i) => !i.ok && !i.healed) ?? [];
  const hKind = !health ? "ring" : bad.length ? "bad" : "solid";
  const hText = !health ? copy.healthUnknown : bad.length ? copy.healthBad : copy.healthOk;

  return (
    <footer className="shrink-0 px-6 pb-[max(1.75rem,env(safe-area-inset-bottom))] pt-6 lg:hidden">
      <div className="mx-auto flex max-w-7xl items-center flex-wrap justify-center gap-x-5 gap-y-2 text-xs text-muted-foreground">
        {inApp && (
          <>
            <Link href={`/${lang}/`} onClick={() => window.dispatchEvent(new Event(NEW_CHAT_EVENT))} className={item}>
              {t(copy.newChat)}
            </Link>
            <span data-chats className="relative shrink-0 lg:hidden">
              <button type="button" aria-expanded={panel} onClick={() => setPanel((p) => !p)} className={item}>
                {t(copy.chats)}
              </button>
              {panel && (
                <div className="absolute bottom-full start-1/2 z-10 mb-3 w-64 -translate-x-1/2 rounded-2xl border bg-background p-2 shadow-lg rtl:translate-x-1/2">
                  <p className="px-2 pb-1 pt-1 text-xs text-muted-foreground">{t(copy.recent)}</p>
                  {chats?.length === 0 && <p className="px-2 py-2 text-xs">{t(copy.noChats)}</p>}
                  <ul>
                    {chats?.map((s) => (
                      <li key={s.id}>
                        <Link
                          href={`/${lang}/?s=${encodeURIComponent(s.id)}`}
                          dir="auto"
                          className="block truncate rounded-lg px-2 py-1.5 text-sm text-foreground transition-colors hover:bg-background"
                        >
                          {s.title || t(copy.newChat)}
                        </Link>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </span>
          </>
        )}
        {/* The same seven parents the sidebar has; the open one reads darker. */}
        {tree.map((n) => (
          <Link
            key={n.id}
            href={n.href(lang)}
            aria-current={trail.parent === n.id ? "page" : undefined}
            className={`${item} ${trail.parent === n.id ? "font-medium text-foreground" : ""}`}
          >
            {typeof n.label === "string" ? n.label : t(n.label)}
          </Link>
        ))}
        <a href={`https://kun.databayt.org/${lang}/docs`} target="_blank" rel="noreferrer" className={item}>{t(copy.docs)}</a>
        <button
          type="button"
          lang={other}
          aria-label={`${t(copy.language)}: ${other === "ar" ? "العربية" : "English"}`}
          onClick={() => {
            storeLang(other);
            router.replace(swapLocale(pathname, window.location.search, other));
          }}
          className={item}
        >
          {other === "ar" ? "العربية" : "English"}
        </button>
        {inApp && (
          <span title={`${t(copy.health)}: ${t(hText)}${bad.length ? ` — ${bad.map((i) => i.key).join(", ")}` : ""}`} className="inline-flex shrink-0 items-center gap-1.5">
            <Dot kind={hKind} />
            <span className="sr-only">{`${t(copy.health)}: ${t(hText)}`}</span>
          </span>
        )}
      </div>
    </footer>
  );
}
