import LaunchersContent from "@/components/launchers/content"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Agents" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function Page({ params }: PageProps) {
  const { lang } = await params
  return <LaunchersContent lang={lang as Locale} />
}
