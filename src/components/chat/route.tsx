"use client"

import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"

import ChatContent from "./content"

/** The home route: the query (`s`/`session`, `prompt`, `task`) is read here, in the client, because the page is a static export. */
export default function ChatRoute({ lang }: { lang: Locale }) {
  const q = useSearchParams()
  const resume = q.get("s") ?? q.get("session") ?? undefined
  const prompt = q.get("prompt") ?? undefined
  const task = q.get("task") ?? undefined
  // Keyed by the resumed session and the starter, so opening another from the menu starts that chat fresh.
  return <ChatContent key={`${resume ?? "new"}|${prompt ?? ""}`} lang={lang} session={resume} prompt={prompt} task={task} />
}
