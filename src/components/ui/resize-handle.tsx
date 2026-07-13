import { ChevronLeft, ChevronRight, GripVertical } from "lucide-react";
import { useRef, type KeyboardEvent, type PointerEvent } from "react";

import { cn } from "@/lib/utils";
import { Button } from "./button";

interface ResizeHandleProps {
  value: number;
  min: number;
  max: number;
  onValueChange: (value: number) => void;
  label: string;
  collapsed?: boolean;
  onCollapsedChange?: (collapsed: boolean) => void;
  collapseLabel?: string;
  expandLabel?: string;
  className?: string;
}

export function ResizeHandle({
  value,
  min,
  max,
  onValueChange,
  label,
  collapsed = false,
  onCollapsedChange,
  collapseLabel,
  expandLabel,
  className,
}: ResizeHandleProps) {
  const drag = useRef<{ pointerId: number; startX: number; startValue: number } | null>(null);

  function resize(next: number) {
    onValueChange(Math.min(max, Math.max(min, next)));
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    if (collapsed && onCollapsedChange) onCollapsedChange(false);
    drag.current = { pointerId: event.pointerId, startX: event.clientX, startValue: value };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!drag.current || drag.current.pointerId !== event.pointerId) return;
    resize(drag.current.startValue + event.clientX - drag.current.startX);
  }

  function handlePointerUp(event: PointerEvent<HTMLDivElement>) {
    if (drag.current?.pointerId !== event.pointerId) return;
    drag.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    if (collapsed && onCollapsedChange) onCollapsedChange(false);
    resize(value + (event.key === "ArrowLeft" ? -16 : 16));
  }

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuemin={onCollapsedChange ? 0 : min}
      aria-valuemax={max}
      aria-valuenow={collapsed ? 0 : value}
      tabIndex={0}
      className={cn(
        "group relative z-10 flex h-full cursor-col-resize touch-none items-center justify-center bg-transparent outline-none",
        className,
      )}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      onKeyDown={handleKeyDown}
    >
      <GripVertical className="pointer-events-none absolute text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" size={12} />
      {onCollapsedChange ? (
        <Button
          variant="secondary"
          size="icon"
          className="absolute top-1/2 z-20 size-5 -translate-y-1/2 rounded-full bg-card p-0 opacity-0 shadow-md transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
          aria-label={collapsed ? expandLabel : collapseLabel}
          title={collapsed ? expandLabel : collapseLabel}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={() => onCollapsedChange(!collapsed)}
        >
          {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
        </Button>
      ) : null}
    </div>
  );
}
