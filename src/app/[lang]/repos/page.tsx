import ReposContent from "@/components/repos/content"
import { type Locale } from "@/components/local/config"

export const metadata = {
  title: "Repos",
}

interface ReposPageProps {
  params: Promise<{ lang: string }>
}

export default async function ReposPage({ params }: ReposPageProps) {
  const { lang } = await params
  return <ReposContent lang={lang as Locale} />
}
