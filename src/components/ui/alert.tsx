import { AlertCircle } from "lucide-react";
import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes } from "react";

import { cn } from "@/lib/utils";
import { Stack } from "./layout";

const alertVariants = cva("flex items-start gap-3 rounded-sm border px-3.5 py-3 text-[13px]", {
  variants: {
    tone: {
      info: "border-primary/20 bg-primary/8 text-primary",
      success: "border-success/20 bg-success/10 text-success",
      warning: "border-warning/25 bg-warning/10 text-warning-foreground",
      danger: "border-destructive/20 bg-destructive/10 text-destructive",
    },
  },
  defaultVariants: { tone: "info" },
});

type AlertProps = HTMLAttributes<HTMLElement> &
  VariantProps<typeof alertVariants> & {
    title?: string;
  };

export function Alert({ title, children, className, tone, ...props }: AlertProps) {
  return (
    <aside className={cn(alertVariants({ tone }), className)} {...props}>
      <AlertCircle size={18} className="mt-0.5 shrink-0" aria-hidden="true" />
      <Stack gap="xs">
        {title ? <strong>{title}</strong> : null}
        <div className="text-sm leading-relaxed text-current">{children}</div>
      </Stack>
    </aside>
  );
}
