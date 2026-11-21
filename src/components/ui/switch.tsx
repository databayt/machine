import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * A small switch, sized like the macOS System Settings toggle. A real `<button role="switch">`:
 * Space and Enter flip it. Monochrome: on is `bg-foreground`, off is a muted track. The thumb
 * slides with logical offsets, so it reads correctly in RTL.
 */
function Switch({
  checked,
  onCheckedChange,
  disabled,
  busy,
  className,
  ...props
}: Omit<React.ComponentProps<"button">, "onClick" | "role" | "type"> & {
  checked: boolean
  onCheckedChange?: (checked: boolean) => void
  /** An action is in flight: the switch holds its value and ignores input. */
  busy?: boolean
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-busy={busy || undefined}
      disabled={disabled || busy}
      data-state={checked ? "checked" : "unchecked"}
      onClick={(e) => {
        e.stopPropagation()
        onCheckedChange?.(!checked)
      }}
      className={cn(
        "relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-0.5 outline-none transition-colors",
        "focus-visible:ring-ring/50 focus-visible:ring-[3px]",
        checked ? "bg-foreground" : "bg-foreground/20",
        busy && "animate-pulse",
        "disabled:cursor-default disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <span
        aria-hidden
        className={cn(
          "pointer-events-none block size-[18px] rounded-full bg-background shadow-sm transition-[margin] duration-150",
          checked ? "ms-4" : "ms-0",
        )}
      />
    </button>
  )
}

export { Switch }
