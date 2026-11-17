"use client"

import { useEffect, useState } from "react"
import Link from "next/link"
import { Trash2Icon } from "lucide-react"

import type { Locale } from "@/components/local/config"
import { OpenInMachine, PageShell, useInMachine } from "../shell/content"
import { listSessions, removeSession } from "./actions"
import { groupSessions, type DayGroup } from "./group"
import type { Session } from "./types"

const COPY = {
  title: { en: "Sessions", ar: "الجلسات" },
  empty: { en: "No sessions yet. Start one from the home screen.", ar: "لا جلسات بعد. ابدأ واحدة من الشاشة الرئيسية." },
  untitled: { en: "New chat", ar: "محادثة جديدة" },
  remove: { en: "Delete", ar: "حذف" },
  sure: { en: "Delete this session?", ar: "حذف هذه الجلسة؟" },
  yes: { en: "Delete", ar: "احذف" },
  no: { en: "Keep", ar: "أبقِ" },
  groups: {
    today: { en: "Today", ar: "اليوم" },
    yesterday: { en: "Yesterday", ar: "أمس" },
    week: { en: "Previous 7 days", ar: "آخر ٧ أيام" },
    older: { en: "Older", ar: "أقدم" },
  } satisfies Record<DayGroup, { en: string; ar: string }>,
}

/** Every chat on this machine, by day, as white rows on the app's gray: open one, or delete it. */
export default function SessionsContent({ lang }: { lang: Locale }): React.JSX.Element | null {
  const isAr = lang === "ar"
  const t = (k: { en: string; ar: string }) => (isAr ? k.ar : k.en)
  const inApp = useInMachine()
  const [sessions, setSessions] = useState<Session[] | null>(null)
  const [confirm, setConfirm] = useState<string | null>(null)

  useEffect(() => {
    if (inApp) void listSessions().then(setSessions).catch(() => setSessions([]))
  }, [inApp])

  if (inApp === null) return null
  if (!inApp) return <OpenInMachine lang={lang} />

  const remove = async (id: string) => {
    await removeSession(id).catch(() => {})
    setSessions((all) => (all ?? []).filter((s) => s.id !== id))
    setConfirm(null)
  }

  return (
    <PageShell lang={lang} title={t(COPY.title)}>
      {sessions?.length === 0 && <p className="text-muted-foreground">{t(COPY.empty)}</p>}
      {sessions === null && (
        <ul aria-hidden className="space-y-2">
          {[0, 1, 2].map((i) => (
            <li key={i} className="h-14 animate-pulse rounded-2xl bg-background" />
          ))}
        </ul>
      )}
      <div className="space-y-6">
        {groupSessions(sessions ?? []).map(({ group, rows }) => (
          <section key={group}>
            <h2 className="pb-2 text-sm text-muted-foreground">{t(COPY.groups[group])}</h2>
            <ul className="space-y-2">
              {rows.map((s) => (
                <li key={s.id} className="flex min-h-14 items-center gap-2 rounded-2xl bg-background px-4">
                  {confirm === s.id ? (
                    <>
                      <span className="flex-1 text-sm">{t(COPY.sure)}</span>
                      <button type="button" onClick={() => void remove(s.id)} className="rounded-full bg-destructive px-3 py-1 text-sm font-medium text-background">
                        {t(COPY.yes)}
                      </button>
                      <button type="button" onClick={() => setConfirm(null)} className="rounded-full bg-muted px-3 py-1 text-sm font-medium">
                        {t(COPY.no)}
                      </button>
                    </>
                  ) : (
                    <>
                      <Link href={`/${lang}/?s=${encodeURIComponent(s.id)}`} className="min-w-0 flex-1 truncate py-4 font-medium">
                        <span dir="auto">{s.title || t(COPY.untitled)}</span>
                      </Link>
                      <button
                        type="button"
                        onClick={() => setConfirm(s.id)}
                        aria-label={`${t(COPY.remove)}: ${s.title || t(COPY.untitled)}`}
                        className="flex size-9 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:text-destructive"
                      >
                        <Trash2Icon className="size-4" aria-hidden />
                      </button>
                    </>
                  )}
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </PageShell>
  )
}
