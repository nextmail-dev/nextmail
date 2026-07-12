import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";

interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  label: string;
}

export function Checkbox({ checked, onCheckedChange, label }: CheckboxProps) {
  return (
    <label className="flex cursor-pointer items-start gap-2.5 text-[13px] leading-relaxed text-foreground">
      <CheckboxPrimitive.Root
        className="mt-0.5 flex size-[18px] shrink-0 items-center justify-center rounded-xs border border-input bg-background text-primary-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/20 data-[state=checked]:border-primary data-[state=checked]:bg-primary"
        checked={checked}
        onCheckedChange={(value) => onCheckedChange(value === true)}
      >
        <CheckboxPrimitive.Indicator>
          <Check size={14} strokeWidth={3} />
        </CheckboxPrimitive.Indicator>
      </CheckboxPrimitive.Root>
      <span>{label}</span>
    </label>
  );
}

