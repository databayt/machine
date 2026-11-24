"use client"

// Brainstorm — `/brainstorm/` lists the seats (ChatGPT, DeepSeek, GLM, Gemini); `?item=<id>` is the live
// site itself, a native webview laid over this page's content box, with one small control to send the
// conversation on. Read in the client (static export). Outside the app there are no webviews, so it
// says "open in Kun", like the Terminal.
import { useRouter, useSearchParams } from "next/navigation"
import { Suspense, useCallback, useEffect, useRef, useState } from "react"

import { cn } from "@/lib/utils"
import type { Locale } from "@/components/local/config"

import { tr } from "../chat/config"
import { promptHref } from "../menu/config"
import { loadTasks } from "../menu/data"
import { useDepartment } from "../menu/use-department"
import { CardGrid, NavCard } from "../shell/cards"
import { BackButton } from "../shell/back"
import { OpenInMachine, useInMachine } from "../shell/content"
import { SHELL_TOP } from "../shell/grid"
import { PageShell } from "../shell/page"
import { webHide, webOpen, webSeats, webStatus, webTranscript, type Rect, type SeatList, type SeatStatus } from "./actions"
import { copy } from "./config"
import { handoffPrompt, transcriptText, type Next } from "./handoff"

export default function Brainstorm({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  const inApp = useInMachine()
  const item = useSearchParams().get("item")
  const [list, setList] = useState<SeatList | null>(null)

  useEffect(() => {
    if (!inApp) return
    webSeats()
      .then(setList)
      .catch(() => setList({ seats: [], missing: true }))
  }, [inApp])

  if (inApp === null) return null
  if (!inApp)
    return (
      <PageShell title={t(copy.title)}>
        <OpenInMachine lang={lang} />
      </PageShell>
    )
  if (item) return <Live key={item} lang={lang} id={item} name={list?.seats.find((s) => s.id === item)?.name ?? item} />
  return (
    <PageShell title={t(copy.title)}>
      {list &&
        (list.seats.length > 0 ? (
          <CardGrid>
            {list.seats.map((s) => (
              <NavCard key={s.id} href={`/${lang}/brainstorm/?item=${encodeURIComponent(s.id)}`} name={s.name} ltr />
            ))}
          </CardGrid>
        ) : (
          <p className="text-muted-foreground">{t(copy.noSeats)}</p>
        ))}
    </PageShell>
  )
}

const same = (a: Rect | null, b: Rect) => !!a && ["x", "y", "width", "height"].every((k) => Math.abs(a[k as keyof Rect] - b[k as keyof Rect]) < 0.5)

/**
 * One seat: the page is the live site. The content box below the strip is measured and handed to Rust,
 * which lays the seat's native webview exactly over it (and hides it when this page is left or the mega
 * menu opens: a native view would otherwise sit above both).
 */
function Live({ lang, id, name }: { lang: Locale; id: string; name: string }) {
  const t = tr(lang === "ar")
  const slot = useRef<HTMLDivElement>(null)
  const last = useRef<Rect | null>(null)
  const menu = useRef(false)
  const [failed, setFailed] = useState(false)
  const [status, setStatus] = useState<SeatStatus | null>(null)

  const place = useCallback(() => {
    const el = slot.current
    if (!el || menu.current) return
    const r = el.getBoundingClientRect()
    const rect: Rect = { x: r.left, y: r.top, width: r.width, height: r.height }
    if (same(last.current, rect) || rect.width < 2 || rect.height < 2) return
    last.current = rect
    webOpen(id, rect).then(
      () => setFailed(false),
      () => {
        last.current = null
        setFailed(true)
      },
    )
  }, [id])

  useEffect(() => {
    place()
    const el = slot.current
    const ro = new ResizeObserver(place)
    if (el) ro.observe(el)
    window.addEventListener("resize", place)
    // A move without a resize (the sidebar settling, a scrollbar) does not wake the observer.
    const tick = setInterval(place, 400)
    const onMenu = (e: Event) => {
      menu.current = !!(e as CustomEvent<{ open: boolean }>).detail?.open
      if (menu.current) {
        last.current = null
        void webHide()
      } else place()
    }
    window.addEventListener("kun:menu", onMenu)
    return () => {
      ro.disconnect()
      clearInterval(tick)
      window.removeEventListener("resize", place)
      window.removeEventListener("kun:menu", onMenu)
      void webHide()
    }
  }, [place])

  // What the adapter says about the page: needs a sign-in, or the free limit is reached.
  useEffect(() => {
    let live = true
    const poll = () =>
      webStatus(id).then(
        (s) => live && setStatus(s),
        () => live && setStatus(null),
      )
    const first = setTimeout(poll, 2500)
    const every = setInterval(poll, 5000)
    return () => {
      live = false
      clearTimeout(first)
      clearInterval(every)
    }
  }, [id])

  const note = failed ? t(copy.failed) : status?.limit ? `${t(copy.limit)}: ${status.limit}` : status?.adapter && status.needsLogin ? t(copy.signIn) : ""

  return (
    <section
      aria-label={name}
      className={cn(
        "flex h-full min-h-0 min-w-0 flex-col ps-[26px] pe-6 pb-6 pt-[max(4.5rem,calc(env(safe-area-inset-top)+3.75rem))] lg:ps-0 lg:pe-16 lg:pb-16",
        SHELL_TOP,
      )}
    >
      <div className="flex min-h-9 items-center gap-4 pb-3">
        <div className="[&_a]:mb-0">
          <Suspense fallback={null}>
            <BackButton />
          </Suspense>
        </div>
        <p role="status" className="min-w-0 flex-1 truncate text-sm text-muted-foreground">
          {note}
        </p>
        <Next lang={lang} id={id} name={name} />
      </div>
      <div ref={slot} className="min-h-0 flex-1 bg-background" />
    </section>
  )
}

const quiet = "shrink-0 whitespace-nowrap text-sm text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"

/**
 * The one control: "Send on", which opens in place (a native view covers anything that would drop over
 * the site) to the three next steps. Each one reads the conversation off the page and opens the chat with
 * a prompt that carries it; the person presses Send there.
 */
function Next({ lang, id, name }: { lang: Locale; id: string; name: string }) {
  const t = tr(lang === "ar")
  const router = useRouter()
  const { dept } = useDepartment()
  const [step, setStep] = useState<"closed" | "menu" | "tasks">("closed")
  const [tasks, setTasks] = useState<{ id: string; title: string }[] | null>(null)
  const [busy, setBusy] = useState(false)
  const [empty, setEmpty] = useState(false)

  const send = async (next: Next, task?: string) => {
    setBusy(true)
    setEmpty(false)
    try {
      const text = transcriptText(await webTranscript(id), name)
      if (!text) return setEmpty(true)
      router.push(promptHref(lang, handoffPrompt(next, name, text, task)))
    } catch {
      setEmpty(true)
    } finally {
      setBusy(false)
    }
  }

  const pickTask = () => {
    setStep("tasks")
    if (tasks) return
    if (!dept) return setTasks([])
    void loadTasks(dept).then((v) => setTasks((v?.board ?? []).map((k) => ({ id: k.id, title: k.title }))))
  }

  if (step === "closed")
    return (
      <button type="button" className={quiet} onClick={() => setStep("menu")}>
        {t(copy.send)}
      </button>
    )
  return (
    <div className="flex min-w-0 flex-wrap items-center justify-end gap-x-4 gap-y-1">
      {empty && <span className="text-sm text-muted-foreground">{t(copy.nothing)}</span>}
      {step === "menu" ? (
        <>
          <button type="button" className={quiet} disabled={busy} onClick={() => void send("idea")}>
            {t(copy.idea)}
          </button>
          <button type="button" className={quiet} disabled={busy} onClick={pickTask}>
            {t(copy.spec)}
          </button>
          <button type="button" className={quiet} disabled={busy} onClick={() => void send("chat")}>
            {t(copy.chat)}
          </button>
        </>
      ) : (
        <select
          autoFocus
          dir="ltr"
          disabled={busy}
          defaultValue=""
          aria-label={t(copy.pickTask)}
          onChange={(e) => e.target.value && void send("spec", e.target.value)}
          className="max-w-64 rounded-md bg-background px-2 py-1 text-sm"
        >
          <option value="">{tasks ? (tasks.length ? t(copy.pickTask) : t(copy.noTasks)) : "…"}</option>
          {(tasks ?? []).map((k) => (
            <option key={k.id} value={k.id}>
              {k.id} · {k.title}
            </option>
          ))}
        </select>
      )}
      <button type="button" className={quiet} onClick={() => setStep("closed")}>
        {t(copy.cancel)}
      </button>
    </div>
  )
}
