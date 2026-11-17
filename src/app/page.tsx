"use client"

import { useEffect } from "react"
import { useRouter } from "next/navigation"

// The root has no content of its own: the app opens in English (the chat swaps to a stored choice).
export default function RootPage() {
  const router = useRouter()
  useEffect(() => router.replace("/en/"), [router])
  return null
}
