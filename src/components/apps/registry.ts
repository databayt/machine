// apps — the readiness steps that are programs on the machine (the foundation, agents and tools
// stages), with a name, a mark and one line each. No React here, so node can test it.
import type { Copy } from "../chat/config.ts"
import type { StepView } from "../readiness/types.ts"

/** The stages the apps live in (shown on Plugins); Setup lists the rest. One list, read by Setup, Plugins and the sidebar. */
export const APP_STAGES: readonly string[] = ["foundation", "agents", "tools"]

/** Config-only checks: they stay in the readiness run but are not apps. */
export const HIDDEN_STEPS: readonly string[] = [
  // The shell launchers are config, set in Settings → Agents.
  "claude.launcher", // catalog ≤ v4
  "launcher.c",
  "launcher.a",
  "launcher.o",
  "launcher.h",
  "opencode.allow",
  "hermes.gateway",
  "agy.eligible",
  "claude.desktop-mcp",
  "tool.whisper-model",
]

export interface AppMeta {
  /** The readiness step id. */
  id: string
  name: string
  /** `/logos/<file>`; absent: a lettered tile. */
  logo?: string
  blurb: Copy
}

const b = (en: string, ar: string): Copy => ({ en, ar })

export const APPS: AppMeta[] = [
  // Foundation
  { id: "brew", name: "Homebrew", logo: "/logos/homebrew.svg", blurb: b("The package manager for the Mac", "مدير الحزم لجهاز ماك") },
  { id: "git", name: "Git", logo: "/logos/git.svg", blurb: b("Version control", "إدارة الإصدارات") },
  { id: "jq", name: "jq", logo: "/logos/jq.svg", blurb: b("Read JSON in the terminal", "قراءة JSON في الطرفية") },
  { id: "uv", name: "uv", logo: "/logos/uv.svg", blurb: b("Python tools, fast", "أدوات بايثون السريعة") },
  { id: "ffmpeg", name: "FFmpeg", logo: "/logos/ffmpeg.svg", blurb: b("Convert audio and video", "تحويل الصوت والفيديو") },
  { id: "tesseract", name: "Tesseract", blurb: b("Read text from images", "قراءة النص من الصور") },
  { id: "tmux", name: "tmux", logo: "/logos/tmux.svg", blurb: b("Keep terminal sessions alive", "إبقاء جلسات الطرفية حيّة") },
  { id: "node", name: "Node.js", logo: "/logos/nodejs.svg", blurb: b("JavaScript runtime", "بيئة تشغيل جافاسكربت") },
  { id: "pnpm", name: "pnpm", logo: "/logos/pnpm.svg", blurb: b("The package manager for projects", "مدير حزم المشاريع") },
  { id: "chrome", name: "Chrome", logo: "/logos/chrome.svg", blurb: b("The browser the agents drive", "المتصفح الذي يقوده الوكلاء") },
  { id: "colima", name: "Colima", blurb: b("Containers without Docker Desktop", "حاويات بلا دوكر ديسكتوب") },
  // Agents
  { id: "claude.cli", name: "Claude Code", logo: "/logos/claude-code.svg", blurb: b("The coding agent in the terminal", "وكيل البرمجة في الطرفية") },
  { id: "claude.desktop", name: "Claude Desktop", logo: "/logos/claude.svg", blurb: b("Claude as a desktop app", "كلود كتطبيق مكتبي") },
  { id: "agy.cli", name: "Antigravity", logo: "/logos/antigravity.svg", blurb: b("Google's coding agent", "وكيل البرمجة من جوجل") },
  { id: "opencode.cli", name: "opencode", logo: "/logos/opencode.svg", blurb: b("An open coding agent", "وكيل برمجة مفتوح") },
  { id: "codex.cli", name: "Codex", logo: "/logos/openai.svg", blurb: b("OpenAI's coding agent", "وكيل البرمجة من أوبن إيه آي") },
  { id: "hermes.cli", name: "Hermes", logo: "/logos/hermes.png", blurb: b("A chat-app agent gateway", "بوابة وكلاء إلى تطبيقات المحادثة") },
  // Tools
  { id: "tool.blender", name: "Blender", logo: "/logos/blender.svg", blurb: b("3D scenes and renders", "مشاهد ثلاثية الأبعاد") },
  { id: "tool.filmcraft", name: "FilmCraft", blurb: b("Edit video by command", "مونتاج الفيديو بالأوامر") },
  { id: "tool.whisper-cli", name: "Whisper", logo: "/logos/openai.svg", blurb: b("Speech to text, offline", "تحويل الكلام إلى نص دون إنترنت") },
  { id: "tool.obs", name: "OBS Studio", logo: "/logos/obs.svg", blurb: b("Record and stream the screen", "تسجيل الشاشة والبث") },
  { id: "tool.comfyui", name: "ComfyUI", logo: "/logos/comfyui.svg", blurb: b("Image generation workflows", "مسارات توليد الصور") },
  { id: "tool.artcraft", name: "ArtCraft", logo: "/logos/artcraft.png", blurb: b("AI art on the desktop", "فن بالذكاء الاصطناعي على سطح المكتب") },
  { id: "tool.webstorm", name: "WebStorm", logo: "/logos/webstorm.svg", blurb: b("JetBrains' JavaScript IDE", "بيئة جافاسكربت من جيت برينز") },
  { id: "tool.vscode", name: "VS Code", logo: "/logos/vscode.png", blurb: b("The code editor", "محرر الأكواد") },
]

export const appMeta = (id: string): AppMeta | undefined => APPS.find((a) => a.id === id)

/** Whether a readiness step is an app the page shows. */
export const isApp = (step: { id: string; stage: string }): boolean => APP_STAGES.includes(step.stage) && !HIDDEN_STEPS.includes(step.id)

/** The steps the page lists: apps that apply here (a `skip` is not for this OS or role, unless the person removed it). */
export const appSteps = (steps: StepView[], removed: string[]): StepView[] =>
  steps.filter((s) => isApp(s) && (s.status !== "skip" || removed.includes(s.id)))

/** On the machine: passing, or only waiting for an update. */
export const isInstalledApp = (s: StepView, removed: string[]): boolean =>
  !removed.includes(s.id) && (s.status === "pass" || s.status === "fixed" || s.status === "updated" || s.status === "update-available")
