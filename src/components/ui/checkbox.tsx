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
        "text-[length:var(--ui-font-control)] flex cursor-pointer items-start gap-2.5 leading-relaxed text-foreground",
        description && "w-fit max-w-full gap-3 rounded-md px-3 py-2.5 transition-colors hover:bg-accent focus-within:bg-accent focus-within:ring-2 focus-within:ring-ring/20 has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-55",
        className,
      )}
    >
      <CheckboxPrimitive.Root
        aria-describedby={description ? `${id}-description` : undefined}
        aria-labelledby={`${id}-label`}
        className="mt-0.5 flex size-[18px] shrink-0 items-center justify-center rounded-sm border-0 bg-background text-primary-foreground shadow-sm ring-1 ring-inset ring-border outline-none focus-visible:ring-3 focus-visible:ring-ring/25 data-[state=checked]:bg-primary data-[state=checked]:ring-primary disabled:cursor-not-allowed"
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
