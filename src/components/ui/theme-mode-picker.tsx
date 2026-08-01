import { Check } from "lucide-react";

import type { ThemePreference } from "@/app/types";
import { cn } from "@/lib/utils";

interface ThemeModeOption {
  value: ThemePreference;
  label: string;
}

interface ThemeModePickerProps {
  label: string;
  value: ThemePreference;
  options: ThemeModeOption[];
  onValueChange: (value: ThemePreference) => void;
  className?: string;
}

export function ThemeModePicker({
  label,
  value,
  options,
  onValueChange,
  className,
}: ThemeModePickerProps) {
  return (
    <fieldset className={cn("m-0 min-w-0 border-0 p-0", className)}>
      <legend className="mb-3 text-xs font-semibold text-foreground">{label}</legend>
      <div className="grid max-w-2xl grid-cols-3 gap-3">
        {options.map((option) => {
          const checked = value === option.value;
          return (
            <label key={option.value} className="group min-w-0 cursor-pointer">
              <input
                className="peer sr-only"
                type="radio"
                name="theme-mode"
                value={option.value}
                checked={checked}
                aria-label={option.label}
                onChange={() => onValueChange(option.value)}
              />
              <span
                className={cn(
                  "relative block overflow-hidden rounded-lg border border-border bg-card p-2 transition-[border-color,box-shadow,transform] duration-150 group-hover:-translate-y-px group-hover:border-foreground/30 peer-focus-visible:ring-3 peer-focus-visible:ring-ring/25",
                  checked && "border-primary shadow-[0_0_0_2px_color-mix(in_srgb,var(--primary)_18%,transparent)]",
                )}
                aria-hidden="true"
              >
                <ThemePreview mode={option.value} />
                {checked ? (
                  <span className="absolute top-3 right-3 grid size-5 place-items-center rounded-full bg-primary text-primary-foreground shadow-sm">
                    <Check size={13} strokeWidth={3} />
                  </span>
                ) : null}
              </span>
              <span className={cn("mt-2 block text-center text-xs text-muted-foreground", checked && "font-semibold text-primary")}>{option.label}</span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}

function ThemePreview({ mode }: { mode: ThemePreference }) {
  if (mode === "system") {
    return (
      <span className="grid h-20 grid-cols-2 overflow-hidden rounded-md border border-border">
        <PreviewPane dark={false} />
        <PreviewPane dark />
      </span>
    );
  }
  return (
    <span className="block h-20 overflow-hidden rounded-md border border-border">
      <PreviewPane dark={mode === "dark"} />
    </span>
  );
}

function PreviewPane({ dark }: { dark: boolean }) {
  return (
    <span className={cn("grid h-full grid-cols-[30%_1fr]", dark ? "bg-slate-900" : "bg-white")}>
      <span className={cn("border-r", dark ? "border-slate-700 bg-slate-800" : "border-slate-200 bg-slate-100")} />
      <span className="flex flex-col gap-2 p-2">
        <span className={cn("h-2 w-2/3 rounded-full", dark ? "bg-slate-500" : "bg-slate-300")} />
        <span className="h-3 w-full rounded bg-primary/75" />
        <span className={cn("h-2 w-4/5 rounded-full", dark ? "bg-slate-600" : "bg-slate-200")} />
      </span>
    </span>
  );
}
