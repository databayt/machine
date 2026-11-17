import { Suspense } from "react"

import CatalogItemRoute from "@/components/catalog/route"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Catalog item" }

export default async function CatalogItemPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <CatalogItemRoute lang={lang as Locale} />
    </Suspense>
  )
}
