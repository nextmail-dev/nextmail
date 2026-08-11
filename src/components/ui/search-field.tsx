import { Search, X } from "lucide-react";
import type { InputHTMLAttributes } from "react";

import { cn } from "@/lib/utils";
import { Button } from "./button";

interface SearchFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "onChange"> {
  clearLabel: string;
  onValueChange: (value: string) => void;
  submitLabel: string;
  onSubmit: () => void;
}

export function SearchField({
  className,
  clearLabel,
  submitLabel,
  value,
  onValueChange,
  onSubmit,
  ...props
}: SearchFieldProps) {
  const hasValue = typeof value === "string" && value.length > 0;
  return (
    <form
      role="search"
      className={cn("flex h-9 w-64 items-center gap-2 rounded-md border border-border/80 bg-input px-3 text-muted-foreground shadow-[var(--shadow-control)] transition-[background-color,border-color,box-shadow] focus-within:border-ring/70 focus-within:bg-card focus-within:ring-2 focus-within:ring-inset focus-within:ring-ring/20", className)}
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <input
        type="search"
        className="text-[length:var(--ui-font-control)] min-w-0 flex-1 appearance-none border-none bg-transparent text-foreground outline-none shadow-none placeholder:text-muted-foreground/70 [&::-webkit-search-cancel-button]:hidden"
        value={value}
        onChange={(event) => onValueChange(event.currentTarget.value)}
        {...props}
      />
      {hasValue ? (
        <Button
          type="button"
          size="icon"
          variant="ghost"
          className="size-6"
          aria-label={clearLabel}
          onClick={() => onValueChange("")}
        >
          <X size={13} />
        </Button>
      ) : null}
      <Button
        type="submit"
        size="icon"
        variant="ghost"
        className="size-7"
        aria-label={submitLabel}
        title={submitLabel}
      >
        <Search size={15} />
      </Button>
    </form>
  );
}
