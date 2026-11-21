"use client"

import Link from "next/link"
import { usePathname, useSearchParams } from "next/navigation"
import { ArrowLeftIcon } from "lucide-react"

import { tr } from "../chat/config"
import { copy } from "../menu/config"
import { backHref } from "./nav"

/**
 * The back button at the top start of every page: one level up the breadcrumb (item → its card →
 * the parent → the chat). The arrow mirrors in Arabic. It reads the address, so it sits under a
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
      aria-label={t(copy.back)}
      className="-ms-2.5 mb-4 inline-flex size-10 items-center justify-center self-start rounded-full text-foreground transition-colors hover:bg-foreground/5"
    >
      <ArrowLeftIcon className="size-5 rtl:-scale-x-100" aria-hidden />
    </Link>
  )
}
