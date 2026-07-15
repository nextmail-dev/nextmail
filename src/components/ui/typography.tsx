import type { HTMLAttributes } from "react";

import { cn } from "@/lib/utils";

export function Text({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      className={cn("m-0 text-sm leading-relaxed text-muted-foreground", className)}
      {...props}
    />
  );
}

export function LabelText({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      className={cn("m-0 text-[length:var(--ui-font-control)] font-semibold tracking-[0.01em] text-foreground", className)}
      {...props}
    />
  );
}

export function Eyebrow({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      className={cn(
        "m-0 text-xs font-bold tracking-[0.12em] text-primary uppercase",
        className,
      )}
      {...props}
    />
  );
}

type HeadingProps = HTMLAttributes<HTMLHeadingElement> & { level?: 1 | 2 | 3 };

export function Heading({ className, level = 1, ...props }: HeadingProps) {
  if (level === 2) {
    return (
      <h2
        className={cn("m-0 text-lg leading-tight font-semibold tracking-tight", className)}
        {...props}
      />
    );
  }
  if (level === 3) {
    return (
      <h3
        className={cn("m-0 text-[15px] leading-tight font-semibold tracking-tight", className)}
        {...props}
      />
    );
  }
  return (
    <h1
      className={cn(
        "m-0 max-w-3xl text-3xl leading-[1.12] font-semibold tracking-[-0.035em] lg:text-[42px]",
        className,
      )}
      {...props}
    />
  );
}
