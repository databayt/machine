import { Suspense } from "react"

import { i18n, type Locale } from "@/components/local/config"
import { MachineMenu } from "@/components/menu/content"
import { LangDir } from "@/components/shell/lang-dir"

// Static export: one tree per locale, nothing else is routable.
export const dynamicParams = false

export function generateStaticParams() {
  return i18n.locales.map((lang) => ({ lang }))
}

// The machine app has no chrome: the page, and one floating two-line button that opens the mega menu.
export default async function MachineLayout({
  children,
  params,
}: {
  children: React.ReactNode
  params: Promise<{ lang: string }>
}) {
  const { lang } = await params
  return (
    <main className="fixed inset-0 overflow-hidden bg-muted">
      <LangDir lang={lang as Locale} />
      {children}
      <Suspense fallback={null}>
        <MachineMenu lang={lang as Locale} />
      </Suspense>
    </main>
  )
}
