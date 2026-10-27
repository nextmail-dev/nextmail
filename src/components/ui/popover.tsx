import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

export const Popover = PopoverPrimitive.Root;
export const PopoverTrigger = PopoverPrimitive.Trigger;

export function PopoverContent({ className, ...props }: ComponentProps<typeof PopoverPrimitive.Content>) {
  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        align="start"
        sideOffset={6}
        collisionPadding={8}
        className={cn("app-floating-content max-h-[var(--radix-popover-content-available-height)] max-w-[calc(100vw-16px)] overflow-hidden rounded-lg border border-border bg-popover p-3 text-popover-foreground shadow-[var(--shadow-float)] outline-none", className)}
        {...props}
      />
    </PopoverPrimitive.Portal>
  );
}
