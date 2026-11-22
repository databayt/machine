import BrainContent from "@/components/brain/content"
import type { Locale } from "@/components/local/config"

export const metadata = { title: "Model" }

interface PageProps {
  params: Promise<{ lang: string }>
}

export default async function Page({ params }: PageProps) {
  const { lang } = await params
  return <BrainContent lang={lang as Locale} />
}
