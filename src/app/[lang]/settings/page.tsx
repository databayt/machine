import { Suspense } from "react"

import SettingsRoute from "@/components/settings/route"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Settings" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function SettingsPage({ params }: PageProps) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <SettingsRoute lang={lang as Locale} />
    </Suspense>
  )
}
