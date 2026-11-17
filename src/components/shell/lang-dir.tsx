"use client"

import { useEffect } from "react"

import { localeConfig, type Locale } from "@/components/local/config"

/** The root layout is static, so the locale's `lang` and `dir` land on <html> once the page mounts. */
export function LangDir({ lang }: { lang: Locale }) {
  useEffect(() => {
    document.documentElement.lang = lang
    document.documentElement.dir = localeConfig[lang].dir
  }, [lang])
  return null
}
