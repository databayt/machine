"use client"

import { Suspense, use } from "react"

import type { Locale } from "@/components/local/config"
import Brainstorm from "@/components/web/content"

export default function BrainstormPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = use(params)
  return (
    <Suspense fallback={null}>
      <Brainstorm lang={lang as Locale} />
    </Suspense>
  )
}
