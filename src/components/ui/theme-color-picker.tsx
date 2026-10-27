import { Check } from "lucide-react";

import { cn } from "@/lib/utils";

export interface ThemeColorOption {
  value: string;
  label: string;
}

interface ThemeColorPickerProps {
  label: string;
  value: string;
  options: ThemeColorOption[];
  onValueChange: (value: string) => void;
  name?: string;
  className?: string;
}

export function ThemeColorPicker({
  label,
  value,
  options,
  onValueChange,
  name = "theme-color",
  className,
}: ThemeColorPickerProps) {
  return (
    <fieldset className={cn("m-0 min-w-0 border-0 p-0", className)}>
      <legend className="mb-3 text-xs font-semibold text-foreground">{label}</legend>
      <div className="flex flex-wrap gap-3">
        {options.map((option) => {
          const checked = value.toLowerCase() === option.value.toLowerCase();
          return (
            <label
              key={option.value}
              className="group relative cursor-default rounded-none"
              title={option.label}
            >
              <input
                className="peer sr-only"
                type="radio"
                name={name}
                value={option.value}
                checked={checked}
                aria-label={option.label}
                onChange={() => onValueChange(option.value)}
              />
              <span
                className={cn(
                  "grid size-8 place-items-center rounded-none border border-foreground/15 transition-opacity duration-150 group-hover:opacity-85 peer-focus-visible:ring-1 peer-focus-visible:ring-ring/70 peer-focus-visible:ring-offset-1 peer-focus-visible:ring-offset-card",
                  checked && "outline outline-1 outline-offset-2 outline-foreground",
                )}
                style={{ backgroundColor: checked ? "var(--primary)" : option.value }}
                aria-hidden="true"
              >
                {checked ? (
                  <Check
                    className="text-primary-foreground"
                    size={17}
                    strokeWidth={3}
                  />
                ) : null}
              </span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}
