import { cn } from "@/lib/utils";

export function Switch({
  checked,
  disabled = false,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={cn(
        "relative h-6 w-11 shrink-0 rounded-full border border-border-strong bg-muted shadow-[var(--shadow-control)] outline-none transition-[background-color,border-color,box-shadow] focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring/70 disabled:cursor-not-allowed disabled:opacity-50",
        checked && "border-primary bg-primary",
      )}
      onClick={() => onCheckedChange(!checked)}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow-sm transition-transform",
          checked && "translate-x-5",
        )}
      />
    </button>
  );
}
