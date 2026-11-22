// connectors — the curated list of connectors, plugins and local MCP servers the databayt team
// works with, in the claude.com marketplace's shape (logo, name, badge, one line). No React here,
// so node can test it. The kun doc `content/docs/connectors.mdx` mirrors this list.
import type { Copy } from "../chat/config.ts"

/**
 * - `connector`: a claude.ai connector (Claude desktop → Settings → Connectors), reaching Code too.
 * - `plugin`: a Claude Code plugin from a marketplace (skills + MCP bundled).
 * - `local`: an MCP server kun registers on the machine (`.claude/mcp.json` → `claude mcp add`).
 */
export type Kind = "connector" | "plugin" | "local"

export type Category = "creative" | "developer" | "productivity" | "business"

export interface Connector {
  id: string
  name: string
  kind: Kind
  category: Category
  /** `/logos/<file>`; brand marks from Simple Icons or the vendor's own icon. Absent: a lettered tile. */
  logo?: string
  blurb: Copy
  /** Names this entry goes by in `claude mcp list`; any match counts as installed. */
  mcp?: string[]
  /** Words in a prompt that route to it. */
  keywords: string[]
  /** Where to read more or add it. */
  url: string
  /** Listed on claude.com/marketplace as Anthropic verified. */
  verified?: boolean
  /** The team uses it every week; shown first, under "Top". */
  top?: boolean
}

export const CATEGORIES: { id: Category; label: Copy }[] = [
  { id: "creative", label: { en: "Creative", ar: "إبداع" } },
  { id: "developer", label: { en: "Developer tools", ar: "أدوات المطوّر" } },
  { id: "productivity", label: { en: "Productivity", ar: "إنتاجية" } },
  { id: "business", label: { en: "Sales & business", ar: "مبيعات وأعمال" } },
]

export const KINDS: { id: Kind; label: Copy }[] = [
  { id: "connector", label: { en: "Connector", ar: "موصل" } },
  { id: "plugin", label: { en: "Plugin", ar: "إضافة" } },
  { id: "local", label: { en: "Local MCP", ar: "MCP محلي" } },
]

const MARKET = "https://claude.com/marketplace"

export const CONNECTORS: Connector[] = [
  // Creative
  {
    id: "blender",
    name: "Blender",
    kind: "local",
    category: "creative",
    logo: "/logos/blender.svg",
    blurb: { en: "Build and render 3D scenes in the live Blender: flythroughs, product shots, the farm film.", ar: "بناء مشاهد ثلاثية الأبعاد وتصييرها في بلندر: جولات طيران ولقطات منتجات." },
    mcp: ["blender"],
    keywords: ["blender", "3d", "flythrough", "بلندر"],
    url: "https://github.com/ahujasid/blender-mcp",
    top: true,
  },
  {
    id: "blender-lab",
    name: "Blender Lab",
    kind: "local",
    category: "creative",
    logo: "/logos/blender.svg",
    blurb: { en: "Blender's official MCP: inspect .blend files, search the API and manual, render thumbnails.", ar: "خادم بلندر الرسمي: فحص ملفات ‎.blend‎ والبحث في التوثيق وتصيير المصغرات." },
    mcp: ["blender-lab"],
    keywords: ["blend file", "blender api"],
    url: "https://projects.blender.org/lab/blender_mcp",
  },
  {
    id: "filmcraft",
    name: "FilmCraft",
    kind: "local",
    category: "creative",
    blurb: { en: "A free non-linear video editor driven by commands: cuts, captions, timelines, previews.", ar: "محرر فيديو مجاني يُدار بالأوامر: قص وترجمة وخط زمني ومعاينة." },
    mcp: ["filmcraft", "filmcraft-desktop"],
    keywords: ["filmcraft", "rough cut", "timeline", "مونتاج"],
    url: "https://github.com/storytold/filmcraft",
    top: true,
  },
  {
    id: "figma",
    name: "Figma",
    kind: "connector",
    category: "creative",
    logo: "/logos/figma.svg",
    blurb: { en: "Read frames into code, push pages back into Figma, diagrams in FigJam.", ar: "تحويل إطارات فيغما إلى كود، وإرسال الصفحات إلى فيغما، ومخططات FigJam." },
    mcp: ["claude.ai Figma", "figma", "figma-desktop"],
    keywords: ["figma", "design", "فيغما"],
    url: `${MARKET}/connectors/figma`,
    verified: true,
    top: true,
  },
  {
    id: "canva",
    name: "Canva",
    kind: "plugin",
    category: "creative",
    logo: "/logos/canva.png",
    blurb: { en: "Search, create, autofill and export designs; brand checks and social resizes.", ar: "البحث في التصاميم وإنشاؤها وتصديرها، وفحص الهوية ومقاسات التواصل." },
    mcp: ["plugin:canva:canva"],
    keywords: ["canva", "كانفا"],
    url: `${MARKET}/connectors/canva`,
    verified: true,
  },
  {
    id: "adobe",
    name: "Adobe for creativity",
    kind: "plugin",
    category: "creative",
    logo: "/logos/adobe.png",
    blurb: { en: "Adobe's creative skills for Claude: imagery, layouts and brand assets.", ar: "مهارات أدوبي الإبداعية لكلود: صور وتخطيطات وأصول الهوية." },
    keywords: ["adobe", "أدوبي"],
    url: `${MARKET}/plugins`,
  },
  {
    id: "after-effects",
    name: "After Effects",
    kind: "local",
    category: "creative",
    logo: "/logos/adobe.png",
    blurb: { en: "Drive Adobe After Effects through its panel: compositions, layers, motion.", ar: "التحكم في أفتر إفكتس عبر لوحته: تراكيب وطبقات وحركة." },
    mcp: ["after-effects"],
    keywords: ["after effects", "motion graphic"],
    url: "https://github.com/JUNKDOGE-JOE/after-effects-mcp",
  },
  {
    id: "premiere-pro",
    name: "Premiere Pro",
    kind: "local",
    category: "creative",
    logo: "/logos/adobe.png",
    blurb: { en: "Drive Adobe Premiere Pro through a bridge panel: sequences, clips, exports.", ar: "التحكم في بريمير برو عبر لوحة وسيطة: تسلسلات ومقاطع وتصدير." },
    mcp: ["premiere-pro"],
    keywords: ["premiere"],
    url: "https://github.com/hetpatel-11/Adobe_Premiere_Pro_MCP",
  },
  {
    id: "claude-design",
    name: "Claude Design",
    kind: "connector",
    category: "creative",
    logo: "/logos/claude.svg",
    blurb: { en: "Anthropic Labs canvas: sync a design system and hand designs off to code.", ar: "لوحة Anthropic Labs: مزامنة نظام التصميم وتسليم التصاميم للكود." },
    mcp: ["claude-design"],
    keywords: ["claude design", "design system"],
    url: "https://claude.ai/design",
  },

  // Developer tools
  {
    id: "github",
    name: "GitHub",
    kind: "plugin",
    category: "developer",
    logo: "/logos/github.svg",
    blurb: { en: "Repositories, issues, Actions and releases across the databayt organization.", ar: "المستودعات والبلاغات وActions والإصدارات في منظمة databayt." },
    mcp: ["github"],
    keywords: ["github", "issue", "push"],
    url: `${MARKET}/plugins/github`,
    verified: true,
    top: true,
  },
  {
    id: "cloudflare",
    name: "Cloudflare",
    kind: "local",
    category: "developer",
    logo: "/logos/cloudflare.svg",
    blurb: { en: "Workers, Containers, DNS and crons: the docs server behind every production deploy.", ar: "Workers وContainers وDNS والمهام المجدولة: مرجع كل نشر للإنتاج." },
    mcp: ["cloudflare-docs"],
    keywords: ["cloudflare", "wrangler", "worker", "deploy"],
    url: "https://developers.cloudflare.com/agents/model-context-protocol/",
    top: true,
  },
  {
    id: "neon",
    name: "Neon",
    kind: "connector",
    category: "developer",
    logo: "/logos/neon.svg",
    blurb: { en: "Postgres projects, branches, migrations and query tuning for every product database.", ar: "مشاريع Postgres وفروعها وترحيلاتها وضبط الاستعلامات لقواعد بيانات المنتجات." },
    mcp: ["Neon", "neon"],
    keywords: ["neon", "prisma", "migration", "database"],
    url: "https://neon.com/docs/ai/neon-mcp-server",
    top: true,
  },
  {
    id: "vercel",
    name: "Vercel",
    kind: "plugin",
    category: "developer",
    logo: "/logos/vercel.svg",
    blurb: { en: "Previews, deployments and logs for the projects still on Vercel.", ar: "المعاينات والنشر والسجلات للمشاريع التي ما زالت على Vercel." },
    mcp: ["vercel"],
    keywords: ["vercel", "preview"],
    url: `${MARKET}/plugins/vercel`,
    verified: true,
  },
  {
    id: "supabase",
    name: "Supabase",
    kind: "connector",
    category: "developer",
    logo: "/logos/supabase.svg",
    blurb: { en: "Databases, auth and storage, when a client project lives on Supabase.", ar: "قواعد البيانات والمصادقة والتخزين لمشاريع العملاء على Supabase." },
    keywords: ["supabase"],
    url: `${MARKET}/connectors/supabase`,
    verified: true,
  },
  {
    id: "sentry",
    name: "Sentry",
    kind: "local",
    category: "developer",
    logo: "/logos/sentry.svg",
    blurb: { en: "Production errors and traces, the first stop in an incident.", ar: "أخطاء الإنتاج وتتبعاتها، أول محطة عند الحوادث." },
    mcp: ["sentry"],
    keywords: ["sentry", "error", "incident"],
    url: "https://docs.sentry.io/product/sentry-mcp/",
  },
  {
    id: "shadcn",
    name: "shadcn/ui",
    kind: "local",
    category: "developer",
    logo: "/logos/shadcnui.svg",
    blurb: { en: "Browse, search and install components from the shadcn registries.", ar: "تصفح مكونات shadcn والبحث عنها وتثبيتها من السجلات." },
    mcp: ["shadcn"],
    keywords: ["shadcn", "component"],
    url: "https://ui.shadcn.com/docs/mcp",
    top: true,
  },
  {
    id: "context7",
    name: "Context7",
    kind: "plugin",
    category: "developer",
    logo: "/logos/context7.png",
    blurb: { en: "Version-specific docs for any library, so code matches the installed release.", ar: "توثيق مطابق لإصدار كل مكتبة، ليطابق الكود النسخة المثبتة." },
    mcp: ["context7"],
    keywords: ["docs", "library"],
    url: `${MARKET}/plugins/context7`,
    verified: true,
  },
  {
    id: "chrome-devtools",
    name: "Chrome DevTools",
    kind: "plugin",
    category: "developer",
    logo: "/logos/googlechrome.svg",
    blurb: { en: "Drive the signed-in Chrome quietly: pages behind a login, network, performance.", ar: "قيادة كروم المسجّل بهدوء: صفحات خلف تسجيل الدخول والشبكة والأداء." },
    mcp: ["chrome-devtools"],
    keywords: ["chrome", "devtools", "lighthouse"],
    url: `${MARKET}/plugins`,
  },
  {
    id: "playwright",
    name: "Playwright",
    kind: "plugin",
    category: "developer",
    logo: "/logos/playwright.png",
    blurb: { en: "A headless browser for QA, screenshots and end-to-end checks.", ar: "متصفح بلا واجهة للفحص واللقطات والاختبارات الشاملة." },
    mcp: ["browser"],
    keywords: ["e2e", "screenshot", "qa"],
    url: `${MARKET}/plugins/playwright`,
    verified: true,
  },
  {
    id: "markitdown",
    name: "MarkItDown",
    kind: "local",
    category: "developer",
    logo: "/logos/markitdown.png",
    blurb: { en: "Turn PDFs, Office files, images, audio and pages into Markdown.", ar: "تحويل ملفات PDF وأوفيس والصور والصوت والصفحات إلى Markdown." },
    mcp: ["markitdown"],
    keywords: ["convert", "pdf", "markdown"],
    url: "https://github.com/microsoft/markitdown",
  },

  // Productivity
  {
    id: "slack",
    name: "Slack",
    kind: "connector",
    category: "productivity",
    logo: "/logos/slack.svg",
    blurb: { en: "Read channels and threads, send messages and canvases to the team.", ar: "قراءة القنوات والسلاسل وإرسال الرسائل واللوحات للفريق." },
    mcp: ["claude.ai Slack", "slack", "plugin:design:slack"],
    keywords: ["slack", "سلاك"],
    url: `${MARKET}/connectors/slack`,
    verified: true,
    top: true,
  },
  {
    id: "gmail",
    name: "Gmail",
    kind: "connector",
    category: "productivity",
    logo: "/logos/gmail.svg",
    blurb: { en: "Search the inbox, summarize threads, draft and send replies.", ar: "البحث في البريد وتلخيص المحادثات وكتابة الردود وإرسالها." },
    mcp: ["claude.ai Gmail"],
    keywords: ["gmail", "email", "بريد"],
    url: `${MARKET}/connectors/gmail`,
    verified: true,
  },
  {
    id: "google-drive",
    name: "Google Drive",
    kind: "connector",
    category: "productivity",
    logo: "/logos/googledrive.svg",
    blurb: { en: "Search, read and upload files; where recordings are mirrored.", ar: "البحث في الملفات وقراءتها ورفعها؛ وفيه تُنسخ التسجيلات." },
    mcp: ["claude.ai Google Drive"],
    keywords: ["drive", "google drive"],
    url: `${MARKET}/connectors/google-drive`,
    verified: true,
  },
  {
    id: "google-calendar",
    name: "Google Calendar",
    kind: "connector",
    category: "productivity",
    logo: "/logos/googlecalendar.svg",
    blurb: { en: "See the schedule and book meetings.", ar: "عرض الجدول وحجز الاجتماعات." },
    mcp: ["claude.ai Google Calendar"],
    keywords: ["calendar", "meeting", "تقويم"],
    url: `${MARKET}/connectors/google-calendar`,
    verified: true,
  },
  {
    id: "notion",
    name: "Notion",
    kind: "connector",
    category: "productivity",
    logo: "/logos/notion.svg",
    blurb: { en: "Search and update the workspace: pages, databases, notes.", ar: "البحث في مساحة العمل وتحديثها: صفحات وقواعد بيانات وملاحظات." },
    mcp: ["notion", "plugin:design:notion"],
    keywords: ["notion"],
    url: `${MARKET}/connectors/notion`,
    verified: true,
  },
  {
    id: "linear",
    name: "Linear",
    kind: "connector",
    category: "productivity",
    logo: "/logos/linear.svg",
    blurb: { en: "Issues, projects and cycles for team workflows.", ar: "البلاغات والمشاريع والدورات لسير عمل الفريق." },
    mcp: ["linear", "plugin:design:linear"],
    keywords: ["linear"],
    url: `${MARKET}/connectors/linear`,
    verified: true,
  },
  {
    id: "atlassian",
    name: "Atlassian",
    kind: "connector",
    category: "productivity",
    logo: "/logos/atlassian.svg",
    blurb: { en: "Jira, Confluence and Bitbucket, for clients who work there.", ar: "Jira وConfluence وBitbucket للعملاء الذين يعملون عليها." },
    mcp: ["plugin:design:atlassian"],
    keywords: ["jira", "confluence"],
    url: `${MARKET}/connectors/atlassian`,
    verified: true,
  },
  {
    id: "airtable",
    name: "Airtable",
    kind: "local",
    category: "productivity",
    logo: "/logos/airtable.svg",
    blurb: { en: "Read and update Airtable bases.", ar: "قراءة قواعد Airtable وتحديثها." },
    mcp: ["airtable"],
    keywords: ["airtable"],
    url: "https://github.com/domdomegg/airtable-mcp-server",
  },

  // Sales & business
  {
    id: "stripe",
    name: "Stripe",
    kind: "local",
    category: "business",
    logo: "/logos/stripe.svg",
    blurb: { en: "Payments, subscriptions and invoices for the products that bill.", ar: "المدفوعات والاشتراكات والفواتير للمنتجات المدفوعة." },
    mcp: ["stripe"],
    keywords: ["stripe", "payment", "billing"],
    url: "https://docs.stripe.com/mcp",
  },
  {
    id: "hubspot",
    name: "HubSpot",
    kind: "connector",
    category: "business",
    logo: "/logos/hubspot.svg",
    blurb: { en: "CRM context for answers and actions; Twenty stays our own CRM.", ar: "سياق إدارة العملاء للأجوبة والإجراءات؛ ويبقى Twenty نظامنا." },
    keywords: ["hubspot", "crm"],
    url: `${MARKET}/connectors/hubspot`,
    verified: true,
  },
  {
    id: "intercom",
    name: "Intercom",
    kind: "connector",
    category: "business",
    logo: "/logos/intercom.svg",
    blurb: { en: "Customer conversations and support history.", ar: "محادثات العملاء وسجل الدعم." },
    mcp: ["plugin:design:intercom"],
    keywords: ["intercom", "support"],
    url: `${MARKET}/connectors`,
  },
  {
    id: "posthog",
    name: "PostHog",
    kind: "local",
    category: "business",
    logo: "/logos/posthog.svg",
    blurb: { en: "Product analytics and feature flags.", ar: "تحليلات المنتج ومفاتيح الميزات." },
    mcp: ["posthog"],
    keywords: ["analytics", "posthog"],
    url: "https://posthog.com/docs/model-context-protocol",
  },
  {
    id: "upwork",
    name: "Upwork",
    kind: "connector",
    category: "business",
    logo: "/logos/upwork.svg",
    blurb: { en: "Find and hire freelancers; post a job.", ar: "البحث عن مستقلين وتوظيفهم ونشر وظيفة." },
    mcp: ["claude.ai Upwork"],
    keywords: ["upwork", "freelancer"],
    url: `${MARKET}/connectors`,
  },
  {
    id: "indeed",
    name: "Indeed",
    kind: "connector",
    category: "business",
    logo: "/logos/indeed.svg",
    blurb: { en: "Search postings and read full job details.", ar: "البحث في الوظائف المعلنة وقراءة تفاصيلها." },
    mcp: ["claude.ai Indeed"],
    keywords: ["jobs", "indeed", "وظائف"],
    url: `${MARKET}/connectors`,
  },
]

/** Installed when any of its MCP names is in the list `claude mcp list` returned. */
export function isInstalled(c: Connector, names: string[]): boolean {
  if (!c.mcp) return false
  const have = new Set(names.map((n) => n.toLowerCase()))
  return c.mcp.some((n) => have.has(n.toLowerCase()))
}

/** A search over name, blurb (both languages) and keywords; empty query matches all. */
export function matches(c: Connector, query: string): boolean {
  const q = query.trim().toLowerCase()
  if (!q) return true
  return [c.name, c.blurb.en, c.blurb.ar, ...c.keywords].some((s) => s.toLowerCase().includes(q))
}
