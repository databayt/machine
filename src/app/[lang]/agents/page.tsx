import AgentsContent from "@/components/readiness/agents"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Agents & tools" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function Page({ params }: PageProps) {
  const { lang } = await params
  return <AgentsContent lang={lang as Locale} />
}
