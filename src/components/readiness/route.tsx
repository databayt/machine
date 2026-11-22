"use client"

import { useSearchParams } from "next/navigation"

import type { Locale } from "@/components/local/config"

import Overview from "./overview"
import StageScreen from "./stage"
import StepScreen from "./step"

/** `/setup/?stage=<id>&step=<id>` — the three levels ride on the query (static export). */
export default function InstallRoute({ lang }: { lang: Locale }) {
  const params = useSearchParams()
  const stage = params.get("stage")
  const step = params.get("step")
  if (stage && step) return <StepScreen key={`${stage}/${step}`} lang={lang} stage={stage} step={step} />
  if (stage) return <StageScreen key={stage} lang={lang} stage={stage} />
  return <Overview lang={lang} />
}
