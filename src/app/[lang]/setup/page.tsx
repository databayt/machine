import { Suspense } from "react"

import InstallRoute from "@/components/readiness/route"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Setup" }

export default async function InstallPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <InstallRoute lang={lang as Locale} />
    </Suspense>
  )
}
