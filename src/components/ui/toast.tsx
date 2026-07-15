import { CheckCircle2, X } from "lucide-react";
import { createPortal } from "react-dom";

import { Button } from "./button";
import { Inline, Stack } from "./layout";
import { LabelText, Text } from "./typography";

interface ToastProps {
  title: string;
  description?: string;
  closeLabel: string;
  onClose: () => void;
}

export function Toast({ title, description, closeLabel, onClose }: ToastProps) {
  return createPortal(
    <aside
      className="fixed top-[calc(var(--titlebar-height)+0.75rem)] right-4 z-[120] w-[min(22rem,calc(100vw-2rem))] rounded-lg bg-popover p-3.5 text-popover-foreground shadow-[0_20px_60px_rgb(15_23_42/0.2)]"
      role="status"
      aria-live="polite"
    >
      <Inline className="items-start">
        <CheckCircle2 className="mt-0.5 shrink-0 text-success" size={18} aria-hidden="true" />
        <Stack className="flex-1" gap="xs">
          <LabelText>{title}</LabelText>
          {description ? <Text className="text-xs">{description}</Text> : null}
        </Stack>
        <Button variant="ghost" size="icon" className="size-7" aria-label={closeLabel} onClick={onClose}>
          <X size={14} />
        </Button>
      </Inline>
    </aside>,
    document.body,
  );
}
