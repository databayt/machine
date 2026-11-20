"use client"

// The machine app shell: the "open in machine" state every machine page shows when it is loaded in a plain browser.
import { useEffect, useState } from "react"
import Link from "next/link"
import { isTauri } from "@tauri-apps/api/core"

import type { Locale } from "@/components/local/config"
import { copy, tr } from "../chat/config"

/** null while unknown (first render), then whether the page runs inside the machine app. */
export function useInMachine(): boolean | null {
  const [inApp, setInApp] = useState<boolean | null>(null)
  useEffect(() => setInApp(isTauri()), [])
  return inApp
}

/** Outside the app: the greeting's place, one sentence, one black pill. */
export function OpenInMachine({ lang }: { lang: Locale }) {
  const t = tr(lang === "ar")
  return (
    <div className="mx-auto flex h-full min-h-[60vh] w-full max-w-3xl flex-col items-center justify-center gap-3 px-4 py-6 text-center">
      <h1 className="text-balance text-[26px] font-semibold leading-tight tracking-tight sm:text-3xl">{t(copy.openTitle)}</h1>
      <p className="max-w-md text-balance text-muted-foreground">{t(copy.openText)}</p>
      <Link
        href={`/${lang}/catalog/`}
        className="mt-3 flex h-12 items-center rounded-full bg-foreground px-6 text-[17px] font-medium text-background transition-colors hover:bg-foreground/90"
      >
        {t(copy.openButton)}
      </Link>
    </div>
  )
}
