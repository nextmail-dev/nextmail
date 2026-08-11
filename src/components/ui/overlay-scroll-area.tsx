import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type UIEvent as ReactUIEvent,
} from "react";

import { cn } from "@/lib/utils";

interface OverlayScrollAreaProps {
  children: ReactNode;
  className?: string;
  contentClassName?: string;
  intrinsic?: boolean;
  onViewportScroll?: (event: ReactUIEvent<HTMLDivElement>) => void;
  style?: CSSProperties;
  trackClassName?: string;
  viewportClassName?: string;
}

interface ScrollbarMetrics {
  scrollable: boolean;
  thumbHeight: number;
  thumbOffset: number;
}

interface ScrollbarDrag {
  pointerId: number;
  startClientY: number;
  startScrollTop: number;
}

const TRACK_INSET = 4;
const MIN_THUMB_HEIGHT = 32;

export function OverlayScrollArea({
  children,
  className,
  contentClassName,
  intrinsic = false,
  onViewportScroll,
  style,
  trackClassName,
  viewportClassName,
}: OverlayScrollAreaProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<ScrollbarDrag | null>(null);
  const [scrollbar, setScrollbar] = useState<ScrollbarMetrics>({
    scrollable: false,
    thumbHeight: 0,
    thumbOffset: 0,
  });

  const measure = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const trackHeight = Math.max(0, viewport.clientHeight - TRACK_INSET * 2);
    const scrollable = viewport.scrollHeight > viewport.clientHeight + 1 && trackHeight > 0;
    const thumbHeight = scrollable
      ? Math.min(trackHeight, Math.max(MIN_THUMB_HEIGHT, trackHeight * viewport.clientHeight / viewport.scrollHeight))
      : 0;
    const availableOffset = Math.max(0, trackHeight - thumbHeight);
    const thumbOffset = scrollable && viewport.scrollHeight > viewport.clientHeight
      ? availableOffset * viewport.scrollTop / (viewport.scrollHeight - viewport.clientHeight)
      : 0;
    setScrollbar({
      scrollable,
      thumbHeight,
      thumbOffset,
    });
  }, []);

  useLayoutEffect(() => {
    measure();
    if (typeof ResizeObserver === "undefined") {
      const handleResize = () => measure();
      window.addEventListener("resize", handleResize);
      return () => window.removeEventListener("resize", handleResize);
    }
    const observer = new ResizeObserver(() => measure());
    if (viewportRef.current) observer.observe(viewportRef.current);
    if (contentRef.current) observer.observe(contentRef.current);
    return () => observer.disconnect();
  }, [measure]);

  useLayoutEffect(() => {
    measure();
  }, [children, measure]);

  function handleScroll(event: ReactUIEvent<HTMLDivElement>) {
    measure();
    onViewportScroll?.(event);
  }

  function handleThumbPointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    const viewport = viewportRef.current;
    if (!viewport) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      pointerId: event.pointerId,
      startClientY: event.clientY,
      startScrollTop: viewport.scrollTop,
    };
  }

  function handleThumbPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const viewport = viewportRef.current;
    const drag = dragRef.current;
    if (!viewport || !drag || drag.pointerId !== event.pointerId) return;
    const trackHeight = Math.max(0, viewport.clientHeight - TRACK_INSET * 2);
    const availableThumbOffset = Math.max(1, trackHeight - scrollbar.thumbHeight);
    const availableScroll = Math.max(0, viewport.scrollHeight - viewport.clientHeight);
    viewport.scrollTop = drag.startScrollTop
      + (event.clientY - drag.startClientY) * availableScroll / availableThumbOffset;
  }

  function handleThumbPointerUp(event: ReactPointerEvent<HTMLDivElement>) {
    if (dragRef.current?.pointerId !== event.pointerId) return;
    dragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  return (
    <div
      className={cn("group/scroll-area relative min-h-0 overflow-hidden", className)}
      data-scrollbar-auto-hide="true"
      style={style}
    >
      <div
        ref={viewportRef}
        className={cn(
          "native-scrollbar-hidden overflow-y-auto",
          intrinsic ? "relative max-h-[inherit]" : "absolute inset-0",
          viewportClassName,
        )}
        onScroll={handleScroll}
      >
        <div ref={contentRef} className={cn("flex min-h-full flex-col", contentClassName)}>
          {children}
        </div>
      </div>
      {scrollbar.scrollable ? (
        <div
          aria-hidden="true"
          className={cn(
            "pointer-events-none absolute inset-y-1 right-1 z-10 w-2.5 transition-opacity",
            "opacity-0 group-hover/scroll-area:opacity-100 group-focus-within/scroll-area:opacity-100",
            trackClassName,
          )}
        >
          <div
            className={cn(
              "group/thumb absolute right-0 flex w-full cursor-default touch-none justify-end",
              "pointer-events-none group-hover/scroll-area:pointer-events-auto group-focus-within/scroll-area:pointer-events-auto",
            )}
            style={{
              height: `${scrollbar.thumbHeight}px`,
              transform: `translateY(${scrollbar.thumbOffset}px)`,
            }}
            onPointerDown={handleThumbPointerDown}
            onPointerMove={handleThumbPointerMove}
            onPointerUp={handleThumbPointerUp}
            onPointerCancel={handleThumbPointerUp}
          >
            <span className="pointer-events-none h-full w-1.5 rounded-full bg-muted-foreground/55 transition-colors group-hover/thumb:bg-muted-foreground/70" />
          </div>
        </div>
      ) : null}
    </div>
  );
}
