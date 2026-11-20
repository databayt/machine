import { Suspense } from "react"

import { i18n, type Locale } from "@/components/local/config"
import { MachineMenu } from "@/components/menu/content"
import { LangDir } from "@/components/shell/lang-dir"
import { ShellFrame } from "@/components/shell/frame"

// Static export: one tree per locale, nothing else is routable.
export const dynamicParams = false

export function generateStaticParams() {
  return i18n.locales.map((lang) => ({ lang }))
}

// The shell: the hamburger (mega menu) on every page; home is the page alone, every other page gets
// the short sidebar beside it on large screens (ShellFrame).
export default async function MachineLayout({
  children,
  params,
}: {
  children: React.ReactNode
  params: Promise<{ lang: string }>
}) {
  const { lang } = await params
  return (
    <main className="fixed inset-0 overflow-hidden bg-muted text-foreground">
      <LangDir lang={lang as Locale} />
      <ShellFrame lang={lang as Locale}>{children}</ShellFrame>
      <Suspense fallback={null}>
        <MachineMenu lang={lang as Locale} />
      </Suspense>
    </main>
  )
}
