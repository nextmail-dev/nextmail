import { forwardRef, useId, type InputHTMLAttributes, type ReactNode } from "react";

import { cn } from "@/lib/utils";

interface CompactFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label: string;
  trailing?: ReactNode;
}

export const CompactField = forwardRef<HTMLInputElement, CompactFieldProps>(function CompactField(
  { label, trailing, className, id: providedId, ...props },
  ref,
) {
  const generatedId = useId();
  const id = providedId ?? generatedId;
  return (
    <label
      htmlFor={id}
      className={cn("flex min-h-11 items-center border-b border-border bg-card", className)}
    >
      <span className="w-20 shrink-0 px-4 text-xs font-semibold text-muted-foreground">{label}</span>
      <input
        ref={ref}
        id={id}
        className="h-10 min-w-0 flex-1 border-0 bg-transparent px-1 text-sm text-foreground outline-none placeholder:text-muted-foreground/60"
        {...props}
      />
      {trailing}
    </label>
  );
});
