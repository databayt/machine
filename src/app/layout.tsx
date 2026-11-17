import "./globals.css"
import type { Metadata } from "next"

import { fontVariables } from "@/components/atom/fonts"
import { cn } from "@/lib/utils"

export const metadata: Metadata = {
  title: "Kun",
  description: "Kun on every teammate's computer.",
}

// `lang` and `dir` are set per locale by <LangDir> (the export is static); Arabic is the default.
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="ar" dir="rtl" suppressHydrationWarning>
      <body className={cn(fontVariables, "overscroll-none antialiased")}>{children}</body>
    </html>
  )
}
