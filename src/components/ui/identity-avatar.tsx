import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function IdentityAvatar({
  label,
  fallback = "?",
  className,
}: {
  label: string;
  fallback?: ReactNode;
  className?: string;
}) {
  const initial = label.trim().charAt(0).toLocaleUpperCase();
  return (
    <span
      data-slot="identity-avatar"
      aria-hidden="true"
      className={cn(
        "grid size-9 shrink-0 place-items-center rounded-full bg-primary [background:var(--primary-gradient)] text-sm font-bold text-primary-foreground",
        className,
      )}
    >
      {initial || fallback}
    </span>
  );
}
