import { Suspense } from "react"

import Connectors from "@/components/connectors/content"
import { type Locale } from "@/components/local/config"

export const metadata = {
  title: "Connectors",
}

export default async function ConnectorsPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <Connectors lang={lang as Locale} />
    </Suspense>
  )
}
