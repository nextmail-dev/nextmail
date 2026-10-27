import { useId, type ReactNode } from "react";

import { cn } from "@/lib/utils";

interface SwitchProps {
  checked: boolean;
  disabled?: boolean;
  label: string;
  id?: string;
  "aria-describedby"?: string;
  onCheckedChange: (checked: boolean) => void;
}

export function Switch({
  checked,
  disabled = false,
  label,
  onCheckedChange,
  ...props
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      {...props}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-border-strong bg-border outline-none transition-colors focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring/70 disabled:cursor-not-allowed disabled:opacity-50",
        checked && "border-primary bg-primary",
      )}
      onClick={() => onCheckedChange(!checked)}
    >
      <span
        aria-hidden="true"
        className={cn(
          "pointer-events-none absolute left-px size-4 rounded-full bg-switch-thumb transition-[transform,background-color]",
          checked && "translate-x-4 bg-primary-foreground",
        )}
      />
    </button>
  );
}

export function SwitchField({
  description,
  className,
  ...props
}: SwitchProps & { description?: ReactNode; className?: string }) {
  const generatedId = useId();
  const id = props.id ?? generatedId;
  return (
    <label
      htmlFor={id}
      className={cn(
        "text-[length:var(--ui-font-control)] flex min-h-11 w-full cursor-default items-center justify-between gap-4 px-3 py-2.5 text-foreground transition-colors hover:bg-accent has-[:focus-visible]:ring-1 has-[:focus-visible]:ring-inset has-[:focus-visible]:ring-ring/70 has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-50",
        className,
      )}
    >
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="font-medium">{props.label}</span>
        {description ? <span id={`${id}-description`} className="text-xs leading-relaxed text-muted-foreground">{description}</span> : null}
      </span>
      <Switch {...props} id={id} aria-describedby={description ? `${id}-description` : props["aria-describedby"]} />
    </label>
  );
}
