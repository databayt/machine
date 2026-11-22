import { Suspense } from "react"

import Machines from "@/components/machines/content"
import { type Locale } from "@/components/local/config"

export const metadata = {
  title: "Machines",
}

export default async function MachinesPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params
  return (
    <Suspense fallback={null}>
      <Machines lang={lang as Locale} />
    </Suspense>
  )
}
