import type { HTMLAttributes } from "react";

import { cn } from "@/lib/utils";

export function Surface({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <section
      className={cn(
        "min-w-0 rounded-lg border border-border/80 bg-card text-card-foreground shadow-[var(--shadow-raised)]",
        className,
      )}
      {...props}
    />
  );
}
