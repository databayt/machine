import ReadinessContent from "@/components/readiness/content"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Readiness" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function Page({ params }: PageProps) {
  const { lang } = await params
  return <ReadinessContent lang={lang as Locale} />
}
