import { X } from "lucide-react";
import { useId, type ReactNode } from "react";

import type { MessageAddress } from "@/app/types";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface RecipientFieldProps {
  label: string;
  addresses: MessageAddress[];
  input: string;
  error?: string | null;
  disabled?: boolean;
  placeholder?: string;
  trailing?: ReactNode;
  onInputChange: (value: string) => void;
  onCommit: () => void;
  onRemove: (index: number) => void;
}

export function RecipientField({
  label,
  addresses,
  input,
  error,
  disabled,
  placeholder,
  trailing,
  onInputChange,
  onCommit,
  onRemove,
}: RecipientFieldProps) {
  const id = useId();
  const errorId = `${id}-error`;
  return (
    <div className={cn("flex min-h-11 items-start bg-card", error && "pb-1")}>
      <label htmlFor={id} className="w-20 shrink-0 px-4 pt-3 text-xs font-semibold text-muted-foreground">
        {label}
      </label>
      <div className="min-w-0 flex-1 py-1.5">
        <div className="flex min-h-8 flex-wrap items-center gap-1.5">
          {addresses.map((address, index) => (
            <span
              key={`${address.email.toLocaleLowerCase()}-${index}`}
              className="inline-flex min-w-0 max-w-full items-center gap-1 rounded-md bg-primary/10 py-1 pr-1 pl-2 text-xs text-primary"
              title={address.name ? `${address.name} <${address.email}>` : address.email}
            >
              <span className="truncate">{address.name || address.email}</span>
              {address.name ? <span className="truncate text-primary/70">&lt;{address.email}&gt;</span> : null}
              {!disabled ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="size-5 shrink-0 text-primary hover:bg-primary/15 hover:text-primary"
                  aria-label={`${label}: ${address.email}`}
                  onClick={() => onRemove(index)}
                >
                  <X size={12} />
                </Button>
              ) : null}
            </span>
          ))}
          <input
            id={id}
            className="h-8 min-w-32 flex-1 appearance-none border-none bg-transparent px-1 text-sm text-foreground shadow-none outline-none ring-0 placeholder:text-muted-foreground/60"
            value={input}
            disabled={disabled}
            inputMode="email"
            autoComplete="off"
            spellCheck={false}
            placeholder={addresses.length ? undefined : placeholder}
            aria-invalid={Boolean(error)}
            aria-describedby={error ? errorId : undefined}
            onChange={(event) => onInputChange(event.currentTarget.value)}
            onBlur={() => { if (input.trim()) onCommit(); }}
            onKeyDown={(event) => {
              const commitSeparator = event.key === "Enter" || event.key === "," || event.key === ";";
              const completeSpace = event.key === " " && input.trim().length > 0;
              if (commitSeparator || completeSpace) {
                event.preventDefault();
                onCommit();
              } else if (event.key === "Backspace" && !input && addresses.length) {
                onRemove(addresses.length - 1);
              }
            }}
          />
        </div>
        {error ? <p id={errorId} className="px-1 pt-1 text-xs text-destructive">{error}</p> : null}
      </div>
      {trailing}
    </div>
  );
}
