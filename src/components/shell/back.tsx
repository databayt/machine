"use client"

import Link from "next/link"
import { usePathname, useSearchParams } from "next/navigation"

import { tr } from "../chat/config"
import { copy } from "../menu/config"
import { backHref } from "./nav"

/**
 * The back button at the top start of every page: one level up the breadcrumb (item → its card →
 * the parent → the chat), as the word Back. It reads the address, so it sits under a
 * `<Suspense>` (PageShell gives it one).
 */
export function BackButton() {
  const pathname = usePathname() ?? ""
  const params = useSearchParams()
  const href = backHref(pathname, params)
  const t = tr(pathname.startsWith("/ar"))
  return (
    <Link
      href={href}
      className="mb-4 self-start text-sm text-muted-foreground transition-colors hover:text-foreground"
    >
      {t(copy.back)}
    </Link>
  )
}
