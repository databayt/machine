import { Suspense } from "react"

import Plugins from "@/components/plugins/content"
import { type Locale } from "@/components/local/config"

export const metadata = {
  title: "Plugins",
}

export default async function PluginsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <Plugins lang={lang as Locale} />
    </Suspense>
  )
}
