import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { PropsWithChildren } from "react";

import { Button } from "./button";
import { cn } from "@/lib/utils";

interface ModalProps extends PropsWithChildren {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  closeLabel: string;
  contentClassName?: string;
}

export function Modal({ open, onOpenChange, title, closeLabel, contentClassName, children }: ModalProps) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/55 backdrop-blur-[2px]" />
        <DialogPrimitive.Content className={cn("fixed top-1/2 left-1/2 z-50 w-[min(520px,calc(100vw-40px))] -translate-x-1/2 -translate-y-1/2 rounded-lg border-0 bg-popover p-6 text-popover-foreground shadow-[0_26px_80px_rgb(15_23_42/0.24)] outline-none", contentClassName)}>
          <DialogPrimitive.Title className="m-0 text-lg font-semibold tracking-tight">
            {title}
          </DialogPrimitive.Title>
          <DialogPrimitive.Close asChild>
            <Button
              variant="ghost"
              size="icon"
              className="absolute top-3 right-3"
              aria-label={closeLabel}
            >
              <X size={17} />
            </Button>
          </DialogPrimitive.Close>
          {children}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
