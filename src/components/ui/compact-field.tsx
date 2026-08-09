import { forwardRef, useId, type InputHTMLAttributes, type ReactNode } from "react";

import { cn } from "@/lib/utils";

interface CompactFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label: string;
  structured?: boolean;
  trailing?: ReactNode;
}

export const CompactField = forwardRef<HTMLInputElement, CompactFieldProps>(function CompactField(
  { label, structured = false, trailing, className, id: providedId, ...props },
  ref,
) {
  const generatedId = useId();
  const id = providedId ?? generatedId;
  return (
    <label
      htmlFor={id}
      className={cn(
        "flex min-h-11 items-center overflow-hidden bg-card",
        structured && "border-b border-border/70",
        className,
      )}
    >
      <span className={cn(
        "shrink-0 px-4 text-xs font-semibold text-muted-foreground",
        structured ? "flex w-24 self-stretch items-center py-2" : "w-20",
      )}>{label}</span>
      <input
        ref={ref}
        id={id}
        className="h-10 min-w-0 flex-1 appearance-none border-none bg-transparent px-1 text-sm text-foreground shadow-none outline-none ring-0 placeholder:text-muted-foreground/60"
        {...props}
      />
      {trailing}
    </label>
  );
});
