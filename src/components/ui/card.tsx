import type { HTMLAttributes } from "react";

import { cn } from "@/lib/utils";

export function Surface({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <section
      className={cn(
        "min-w-0 rounded-lg border-0 bg-card text-card-foreground shadow-[0_10px_30px_rgb(15_23_42/0.06)]",
        className,
      )}
      {...props}
    />
  );
}
