"use client"

import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"

import ReposContent from "../repos/content"
import AgentsContent from "./agents"
import ReadinessContent from "./content"

/** `/install/?stage=<id>&step=<id>` — the levels ride on the query (static export). Interim: two special leaves, the rest is the readiness list. */
export default function InstallRoute({ lang }: { lang: Locale }) {
  const params = useSearchParams()
  const stage = params.get("stage")
  const step = params.get("step")
  if (stage === "repos" && step === "all") return <ReposContent lang={lang} />
  if (stage === "agents" && step === "default") return <AgentsContent lang={lang} />
  return <ReadinessContent lang={lang} />
}
