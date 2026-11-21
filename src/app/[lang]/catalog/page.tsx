import { Suspense } from "react"

import CatalogRoute from "@/components/catalog/route"
import { type Locale } from "@/components/local/config"

export const metadata = {
  title: ".claude",
}

export default async function CatalogPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <CatalogRoute lang={lang as Locale} />
    </Suspense>
  )
}
