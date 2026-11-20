import HealthContent from "@/components/update/content"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Health & updates" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function Page({ params }: PageProps) {
  const { lang } = await params
  return <HealthContent lang={lang as Locale} />
}
