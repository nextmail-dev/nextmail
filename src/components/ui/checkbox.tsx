import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { useId, type ReactNode } from "react";

import { cn } from "@/lib/utils";

interface CheckboxProps {
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
  label: string;
  description?: ReactNode;
  className?: string;
}

export function Checkbox({
  checked,
  disabled = false,
  onCheckedChange,
  label,
  description,
  className,
}: CheckboxProps) {
  const id = useId();

  return (
    <label
      className={cn(
        "text-[length:var(--ui-font-control)] flex cursor-default items-start gap-2.5 leading-relaxed text-foreground",
        description && "w-full max-w-full gap-3 rounded-md px-3 py-2.5 transition-colors hover:bg-accent focus-within:bg-accent focus-within:ring-1 focus-within:ring-inset focus-within:ring-ring/60 has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-55",
        className,
      )}
    >
      <CheckboxPrimitive.Root
        aria-describedby={description ? `${id}-description` : undefined}
        aria-labelledby={`${id}-label`}
        className="mt-0.5 flex size-[18px] shrink-0 items-center justify-center rounded-sm border border-border-strong bg-background text-primary-foreground shadow-[var(--shadow-control)] outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring/70 data-[state=checked]:border-primary data-[state=checked]:bg-primary disabled:cursor-not-allowed"
        checked={checked}
        disabled={disabled}
        onCheckedChange={(value) => onCheckedChange(value === true)}
      >
        <CheckboxPrimitive.Indicator>
          <Check size={14} strokeWidth={3} />
        </CheckboxPrimitive.Indicator>
      </CheckboxPrimitive.Root>
      <span className={cn(description && "flex min-w-0 flex-1 flex-col gap-1.5")}>
        <span id={`${id}-label`}>{label}</span>
        {description ? (
          <span id={`${id}-description`} className="text-xs leading-relaxed text-muted-foreground">
            {description}
          </span>
        ) : null}
      </span>
    </label>
  );
}
