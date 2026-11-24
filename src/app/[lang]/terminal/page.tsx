"use client"

import { use } from "react"

import type { Locale } from "@/components/local/config"
import TerminalContent from "@/components/terminal/content"

export default function TerminalPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = use(params)
  return <TerminalContent lang={lang as Locale} />
}
