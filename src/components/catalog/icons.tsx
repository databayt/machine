import { ArrowDown, Plug, User } from "lucide-react"
import { useId } from "react"

import { cn } from "@/lib/utils"

/** What the small mark on a document stands for. */
export type Glyph = "md" | "sh" | "json" | "env" | "mcp"

interface IconProps {
  /** Available, not on this computer: reduced opacity and a download badge. */
  dimmed?: boolean
  /** Disabled: greyscale. */
  off?: boolean
  /** Yours, not kun's: a small person badge. */
  personal?: boolean
  /** Has an update: a small dot. */
  update?: boolean
  className?: string
}

/** The little round badge at the icon's corner. */
function Badge({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <span
      className={cn(
        "absolute -bottom-0.5 -end-0.5 flex size-5 items-center justify-center rounded-full bg-foreground text-background ring-2 ring-muted",
        className,
      )}
    >
      {children}
    </span>
  )
}

function Frame({ dimmed, off, personal, update, className, children }: IconProps & { children: React.ReactNode }) {
  return (
    <span
      aria-hidden
      className={cn(
        "relative inline-block size-14 shrink-0 sm:size-16",
        className,
      )}
    >
      <span className={cn("block size-full", dimmed && "opacity-45", off && "opacity-70 grayscale")}>{children}</span>
      {dimmed && (
        <Badge>
          <ArrowDown className="size-3" strokeWidth={3} />
        </Badge>
      )}
      {personal && (
        <Badge>
          <User className="size-3" strokeWidth={2.5} />
        </Badge>
      )}
      {update && <span className="absolute -end-0.5 -top-0.5 size-3 rounded-full bg-sky-500 ring-2 ring-muted" />}
    </span>
  )
}

/** A folder: a darker back panel with a tab, a lighter front panel, soft corners. */
export function FolderIcon(props: IconProps) {
  const id = useId()
  return (
    <Frame {...props}>
      <svg viewBox="0 0 64 64" className="size-full" fill="none">
        <defs>
          <linearGradient id={`${id}b`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#5aa9f0" />
            <stop offset="1" stopColor="#3a86d8" />
          </linearGradient>
          <linearGradient id={`${id}f`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#8ccbff" />
            <stop offset="1" stopColor="#59aef5" />
          </linearGradient>
        </defs>
        <path
          d="M6 14a5 5 0 0 1 5-5h12.2a5 5 0 0 1 3.5 1.4l2.6 2.5a5 5 0 0 0 3.5 1.4H53a5 5 0 0 1 5 5V48a5 5 0 0 1-5 5H11a5 5 0 0 1-5-5V14Z"
          fill={`url(#${id}b)`}
        />
        <rect x="6" y="20" width="52" height="33" rx="5" fill={`url(#${id}f)`} />
        <path d="M11 20.5h42" stroke="#fff" strokeOpacity=".45" strokeLinecap="round" />
      </svg>
    </Frame>
  )
}

/** A document: a white sheet, a folded top corner, a hairline border, a mark in the lower half. */
export function DocIcon({ glyph = "md", ...props }: IconProps & { glyph?: Glyph }) {
  return (
    <Frame {...props}>
      <svg viewBox="0 0 64 64" className="size-full" fill="none">
        <path
          d="M14 8.5A3.5 3.5 0 0 1 17.5 5H40l14 14v35.5a3.5 3.5 0 0 1-3.5 3.5h-33A3.5 3.5 0 0 1 14 54.5v-46Z"
          className="fill-white stroke-black/20 dark:fill-neutral-100 dark:stroke-white/30"
          strokeWidth="1"
        />
        <path d="M40 5v11.5a2.5 2.5 0 0 0 2.5 2.5H54L40 5Z" className="fill-neutral-200 stroke-black/20" strokeWidth="1" strokeLinejoin="round" />
        <path d="M21 25h13M21 30h22" stroke="#000" strokeOpacity=".12" strokeWidth="2" strokeLinecap="round" />
        {glyph === "mcp" ? null : (
          <text
            x="34"
            y="48"
            direction="ltr"
            unicodeBidi="isolate"
            textAnchor="middle"
            fontSize="10"
            fontWeight="700"
            fontFamily="ui-monospace, SFMono-Regular, Menlo, monospace"
            fill="#3a3a3c"
          >
            {glyphText[glyph]}
          </text>
        )}
      </svg>
      {glyph === "mcp" && (
        <span className="absolute inset-x-0 bottom-[14%] flex justify-center text-neutral-700">
          <Plug className="size-[22px]" strokeWidth={2} />
        </span>
      )}
    </Frame>
  )
}

const glyphText: Record<Exclude<Glyph, "mcp">, string> = { md: ".md", sh: ".sh", json: "{ }", env: ".env" }
