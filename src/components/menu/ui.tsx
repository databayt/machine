import { LockIcon } from "lucide-react";

import { cn } from "@/lib/utils";

export const quiet = "text-sm text-muted-foreground";

/** A status dot in tokens only: solid = good/ready, ring = unknown/missing, destructive = bad. */
export function Dot({ kind, className }: { kind: "solid" | "ring" | "bad" | "lock"; className?: string }) {
  if (kind === "lock") return <LockIcon aria-hidden className={cn("size-3.5 shrink-0 text-muted-foreground", className)} />;
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-2 shrink-0 rounded-full",
        kind === "solid" && "bg-foreground",
        kind === "bad" && "bg-destructive",
        kind === "ring" && "border border-muted-foreground",
        className,
      )}
    />
  );
}
