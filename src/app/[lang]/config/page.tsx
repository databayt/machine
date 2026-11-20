import ConfigContent from "@/components/config/content"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Configuration" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function Page({ params }: PageProps) {
  const { lang } = await params
  return <ConfigContent lang={lang as Locale} />
}
