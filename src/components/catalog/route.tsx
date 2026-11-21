"use client"

import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"

import Folder from "./folder"
import Item from "./item"
import Mcp from "./mcp"
import Root from "./root"
import { useTree } from "./use-tree"

const DIRS = ["skills", "agents", "rules", "hooks", "departments"]

/** `/catalog/` → `?dir=<dir>` → `?dir=<dir>&item=<id>`; read in the client (static export). */
export default function CatalogRoute({ lang }: { lang: Locale }) {
  const params = useSearchParams()
  const dir = params.get("dir") ?? ""
  const id = params.get("item")
  const state = useTree()

  if (dir === "mcp") return <Mcp lang={lang} name={id?.startsWith("mcp:") ? id.slice(4) : undefined} />
  if (id) return <Item lang={lang} id={id} state={state} />
  if (DIRS.includes(dir)) return <Folder lang={lang} dir={dir} state={state} />
  return <Root lang={lang} state={state} />
}
