"use client"

import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"

import CatalogContent from "./content"
import CatalogDetail from "./detail"

/** `/catalog/?dir=<folder>&item=<type>:<name>` — `item` opens one registry item, else the list; read in the client (static export). */
export default function CatalogRoute({ lang }: { lang: Locale }) {
  const params = useSearchParams()
  const id = params.get("item")
  if (!id) return <CatalogContent key={params.get("dir") ?? ""} lang={lang} />
  const i = id.indexOf(":")
  return <CatalogDetail lang={lang} type={i < 0 ? id : id.slice(0, i)} name={i < 0 ? "" : id.slice(i + 1)} />
}
