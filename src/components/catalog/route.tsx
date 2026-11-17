"use client"

import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"

import CatalogDetail from "./detail"

/** `/catalog/item/?id=<type>:<name>` — one registry item; the id is read in the client (static export). */
export default function CatalogItemRoute({ lang }: { lang: Locale }) {
  const id = useSearchParams().get("id") ?? ""
  const i = id.indexOf(":")
  return <CatalogDetail lang={lang} type={i < 0 ? id : id.slice(0, i)} name={i < 0 ? "" : id.slice(i + 1)} />
}
