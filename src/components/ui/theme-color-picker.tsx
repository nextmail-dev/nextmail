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
              className="group relative cursor-pointer rounded-full"
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
                  "grid size-8 place-items-center rounded-full shadow-[inset_0_0_0_1px_rgb(255_255_255/0.24)] transition-[transform,box-shadow] duration-150 group-hover:scale-105 peer-focus-visible:ring-3 peer-focus-visible:ring-ring/30 peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-card",
                  checked && "shadow-[inset_0_0_0_1px_rgb(255_255_255/0.3),0_0_0_2px_var(--card),0_0_0_4px_var(--foreground)]",
                )}
                style={{ backgroundColor: option.value }}
                aria-hidden="true"
              >
                {checked ? (
                  <Check
                    className="text-white drop-shadow-[0_1px_1px_rgb(0_0_0/0.35)]"
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
