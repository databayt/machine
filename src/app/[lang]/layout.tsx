import { Suspense } from "react"

import { i18n, type Locale } from "@/components/local/config"
import { MachineMenu } from "@/components/menu/content"
import { LangDir } from "@/components/shell/lang-dir"
import { AppSidebar } from "@/components/shell/sidebar"

// Static export: one tree per locale, nothing else is routable.
export const dynamicParams = false

export function generateStaticParams() {
  return i18n.locales.map((lang) => ({ lang }))
}

// The shell: on large screens the sidebar (the mega menu's Departments column) beside the page, laid out
// like the menu's grid; below that, the page alone and one floating two-line button for the mega menu.
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
      <div className="grid h-full lg:grid-cols-[12rem_minmax(0,1fr)] lg:gap-x-28 lg:ps-[46px]">
        <AppSidebar lang={lang as Locale} />
        <div className="h-full min-w-0 overflow-y-auto overscroll-contain">{children}</div>
      </div>
      <Suspense fallback={null}>
        <MachineMenu lang={lang as Locale} />
      </Suspense>
    </main>
  )
}
