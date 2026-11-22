// connectors page — every string the page shows, ar + en.
import type { Copy } from "../chat/config.ts"

export const copy = {
  title: { en: "Connectors", ar: "الموصلات" },
  search: { en: "Search connectors, plugins and MCP servers", ar: "ابحث في الموصلات والإضافات وخوادم MCP" },
  all: { en: "All", ar: "الكل" },
  top: { en: "Top", ar: "الأبرز" },
  add: { en: "Add", ar: "أضف" },
  remove: { en: "Remove", ar: "أزل" },
  manage: { en: "Manage", ar: "إدارة" },
  more: { en: "More for", ar: "خيارات" },
  open: { en: "Open", ar: "افتح" },
  empty: { en: "Nothing matches.", ar: "لا نتائج مطابقة." },
  notFound: { en: "Not found.", ar: "غير موجود." },
  keywords: { en: "Say any of these and the prompt routes here", ar: "اذكر أياً منها فيتجه الطلب إلى هنا" },
  mcpNames: { en: "MCP names", ar: "أسماء MCP" },
  type: { en: "Type", ar: "النوع" },
  category: { en: "Category", ar: "الفئة" },
} satisfies Record<string, Copy>
