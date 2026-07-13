import type { PropsWithChildren } from "react";

import { cn } from "@/lib/utils";

export function IconTile({ children, large }: PropsWithChildren<{ large?: boolean }>) {
  return (
    <span
      className={cn(
        "flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary",
        large && "size-13",
      )}
      aria-hidden="true"
    >
      {children}
    </span>
  );
}

export function StatusDot() {
  return <span className="size-2 rounded-full bg-success shadow-[0_0_0_4px_var(--success-soft)]" aria-hidden="true" />;
}

export function UnreadDot() {
  return <span className="size-2 shrink-0 rounded-full bg-primary" aria-hidden="true" />;
}
