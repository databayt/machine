"use client"

import { use } from "react"

import type { Locale } from "@/components/local/config"
import SessionsContent from "@/components/sessions/content"

export default function SessionsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = use(params)
  return <SessionsContent lang={lang as Locale} />
}
