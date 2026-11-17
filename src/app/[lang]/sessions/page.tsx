"use client"

import { use, useEffect } from "react"
import { useRouter } from "next/navigation"

// Sessions live in the mega menu now.
export default function SessionsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = use(params)
  const router = useRouter()
  useEffect(() => router.replace(`/${lang}/`), [lang, router])
  return null
}
