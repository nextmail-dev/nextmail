import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import { useId } from "react";

import { cn } from "@/lib/utils";

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectFieldProps {
  label: string;
  value: string;
  options: SelectOption[];
  onValueChange: (value: string) => void;
  compact?: boolean;
  className?: string;
  disabled?: boolean;
  triggerClassName?: string;
}

export function SelectField({
  label,
  value,
  options,
  onValueChange,
  compact,
  className,
  disabled,
  triggerClassName,
}: SelectFieldProps) {
  const id = useId();
  return (
    <div className={cn("flex min-w-0 flex-1 flex-col gap-1.5", compact && "flex-none", className)}>
      <label className={cn("text-xs font-semibold", compact && "sr-only")} id={`${id}-label`}>
        {label}
      </label>
      <SelectPrimitive.Root value={value} onValueChange={onValueChange} disabled={disabled}>
        <SelectPrimitive.Trigger
          className={cn(
            "text-[length:var(--ui-font-control)] flex h-10 w-full min-w-36 items-center justify-between gap-3 rounded-md border border-border/80 bg-input px-3 shadow-[var(--shadow-control)] outline-none transition-[background-color,border-color,box-shadow] focus:border-ring/70 focus:bg-card focus:ring-2 focus:ring-inset focus:ring-ring/20 disabled:cursor-not-allowed disabled:opacity-50",
            compact && "h-8 min-w-32 bg-muted px-2.5",
            triggerClassName,
          )}
          aria-labelledby={`${id}-label`}
        >
          <SelectPrimitive.Value />
          <SelectPrimitive.Icon>
            <ChevronDown size={15} />
          </SelectPrimitive.Icon>
        </SelectPrimitive.Trigger>
        <SelectPrimitive.Portal>
          <SelectPrimitive.Content
            className="app-floating-content max-h-[var(--radix-select-content-available-height)] min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-md border border-border/80 bg-popover p-1.5 text-popover-foreground shadow-[var(--shadow-float)]"
            position="popper"
            sideOffset={5}
          >
            <SelectPrimitive.Viewport className="app-select-viewport max-h-[calc(var(--radix-select-content-available-height)-0.75rem)] overflow-y-auto overscroll-contain">
              {options.map((option) => (
                <SelectPrimitive.Item
                  className="text-[length:var(--ui-font-control)] relative flex h-8 cursor-default items-center rounded-xs py-0 pr-8 pl-2.5 outline-none select-none focus:bg-accent focus:text-accent-foreground"
                  value={option.value}
                  key={option.value}
                >
                  <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
                  <SelectPrimitive.ItemIndicator className="absolute right-2 text-primary">
                    <Check size={15} />
                  </SelectPrimitive.ItemIndicator>
                </SelectPrimitive.Item>
              ))}
            </SelectPrimitive.Viewport>
          </SelectPrimitive.Content>
        </SelectPrimitive.Portal>
      </SelectPrimitive.Root>
    </div>
  );
}
