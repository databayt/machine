"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { ActivityIcon, CogIcon, FolderGit2Icon, LanguagesIcon, SquarePenIcon } from "lucide-react";

import type { Locale } from "@/components/local/config";
import { tr } from "../chat/config";
import { storeLang, swapLocale } from "../shell/lang";
import { copy, NEW_CHAT_EVENT } from "./config";
import type { HealthReport } from "./data";
import { Dot } from "./ui";

const button =
  "relative inline-flex size-11 items-center justify-center rounded-full text-muted-foreground outline-none transition-colors hover:bg-background hover:text-foreground focus-visible:bg-background focus-visible:text-foreground";

/**
 * The menu's last row on large screens (it replaces the footer line there): four outline icons —
 * New chat, Repos, Engine (or the health check where Engine is not allowed) and the language switch.
 * The machine's health rides on the Engine icon as a small dot.
 */
export function Tools({
  lang,
  inApp,
  showEngine,
  health,
}: {
  lang: Locale;
  inApp: boolean;
  showEngine: boolean;
  health: HealthReport | null;
}): React.JSX.Element {
  const isAr = lang === "ar";
  const t = tr(isAr);
  const router = useRouter();
  const pathname = usePathname();
  const other: Locale = isAr ? "en" : "ar";
  const bad = health?.items.filter((i) => !i.ok && !i.healed) ?? [];
  const hKind = !health ? "ring" : bad.length ? "bad" : "solid";
  const hText = t(!health ? copy.healthUnknown : bad.length ? copy.healthBad : copy.healthOk);
  const healthTitle = `${t(copy.health)}: ${hText}${bad.length ? ` — ${bad.map((i) => i.key).join(", ")}` : ""}`;
  const icon = "size-5";

  return (
    <div className="flex items-center gap-1 lg:-ms-3">
      <Link
        href={`/${lang}/`}
        onClick={() => window.dispatchEvent(new Event(NEW_CHAT_EVENT))}
        aria-label={t(copy.newChat)}
        title={t(copy.newChat)}
        className={button}
      >
        <SquarePenIcon className={icon} strokeWidth={1.75} aria-hidden />
      </Link>
      {inApp && (
        <Link href={`/${lang}/install/?stage=repos&step=all`} aria-label={t(copy.repos)} title={t(copy.repos)} className={button}>
          <FolderGit2Icon className={icon} strokeWidth={1.75} aria-hidden />
        </Link>
      )}
      {showEngine ? (
        <Link href={`/${lang}/engine/`} aria-label={healthTitle} title={`${t(copy.engine)} · ${healthTitle}`} className={button}>
          <CogIcon className={icon} strokeWidth={1.75} aria-hidden />
          {inApp && <Dot kind={hKind} className="absolute end-2 top-2" />}
        </Link>
      ) : (
        inApp && (
          <span role="img" aria-label={healthTitle} title={healthTitle} className={button}>
            <ActivityIcon className={icon} strokeWidth={1.75} aria-hidden />
            <Dot kind={hKind} className="absolute end-2 top-2" />
          </span>
        )
      )}
      <button
        type="button"
        aria-label={`${t(copy.language)}: ${other === "ar" ? "العربية" : "English"}`}
        title={other === "ar" ? "العربية" : "English"}
        onClick={() => {
          storeLang(other);
          router.replace(swapLocale(pathname, window.location.search, other));
        }}
        className={button}
      >
        <LanguagesIcon className={icon} strokeWidth={1.75} aria-hidden />
      </button>
    </div>
  );
}
