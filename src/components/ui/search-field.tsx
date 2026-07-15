import { Search, X } from "lucide-react";
import type { InputHTMLAttributes } from "react";

import { cn } from "@/lib/utils";
import { Button } from "./button";

interface SearchFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "onChange"> {
  clearLabel: string;
  onValueChange: (value: string) => void;
}

export function SearchField({ className, clearLabel, value, onValueChange, ...props }: SearchFieldProps) {
  const hasValue = typeof value === "string" && value.length > 0;
  return (
    <label className={cn("flex h-9 w-64 items-center gap-2 rounded-md bg-muted px-3 text-muted-foreground focus-within:ring-3 focus-within:ring-ring/20", className)}>
      <Search size={16} aria-hidden="true" />
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
    </label>
  );
}
