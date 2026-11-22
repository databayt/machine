import { Suspense } from "react"

import Apps from "@/components/apps/content"
import { type Locale } from "@/components/local/config"

export const metadata = {
  title: "Apps",
}

export default async function AppsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <Apps lang={lang as Locale} />
    </Suspense>
  )
}
