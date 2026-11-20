import { Suspense } from "react"

import ChatRoute from "@/components/chat/route"
import type { Locale } from "@/components/local/config"
import { LaunchGate } from "@/components/readiness/launch"

export const metadata = { title: "Kun" }

export default async function HomePage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <LaunchGate lang={lang as Locale} />
      <ChatRoute lang={lang as Locale} />
    </Suspense>
  )
}
