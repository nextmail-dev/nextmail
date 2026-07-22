import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";

import { cn } from "@/lib/utils";

interface OverlayScrollAreaProps {
  children: ReactNode;
  className?: string;
  contentClassName?: string;
  style?: CSSProperties;
  trackClassName?: string;
  viewportClassName?: string;
  alwaysVisible?: boolean;
}

interface ScrollbarMetrics {
  active: boolean;
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
  style,
  trackClassName,
  viewportClassName,
  alwaysVisible = false,
}: OverlayScrollAreaProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const hideTimerRef = useRef<number | null>(null);
  const dragRef = useRef<ScrollbarDrag | null>(null);
  const [scrollbar, setScrollbar] = useState<ScrollbarMetrics>({
    active: false,
    scrollable: false,
    thumbHeight: 0,
    thumbOffset: 0,
  });

  const measure = useCallback((active?: boolean) => {
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
    setScrollbar((current) => ({
      active: active ?? current.active,
      scrollable,
      thumbHeight,
      thumbOffset,
    }));
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

  useLayoutEffect(() => () => {
    if (hideTimerRef.current !== null) window.clearTimeout(hideTimerRef.current);
  }, []);

  function handleScroll() {
    if (hideTimerRef.current !== null) window.clearTimeout(hideTimerRef.current);
    measure(true);
    hideTimerRef.current = window.setTimeout(() => {
      setScrollbar((current) => ({ ...current, active: false }));
      hideTimerRef.current = null;
    }, 700);
  }

  function handleThumbPointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    const viewport = viewportRef.current;
    if (!viewport) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    if (hideTimerRef.current !== null) window.clearTimeout(hideTimerRef.current);
    dragRef.current = {
      pointerId: event.pointerId,
      startClientY: event.clientY,
      startScrollTop: viewport.scrollTop,
    };
    setScrollbar((current) => ({ ...current, active: true }));
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
    if (hideTimerRef.current !== null) window.clearTimeout(hideTimerRef.current);
    hideTimerRef.current = window.setTimeout(() => {
      setScrollbar((current) => ({ ...current, active: false }));
      hideTimerRef.current = null;
    }, 700);
  }

  return (
    <div className={cn("relative min-h-0", className)} style={style}>
      <div
        ref={viewportRef}
        className={cn("native-scrollbar-hidden absolute inset-0 overflow-y-auto", viewportClassName)}
        onScroll={handleScroll}
      >
        <div ref={contentRef} className={cn("flex min-h-full flex-col", contentClassName)}>
          {children}
        </div>
      </div>
      {scrollbar.scrollable ? (
        <div className={cn("pointer-events-none absolute inset-y-1 right-1 z-10 w-1.5", trackClassName)}>
          <div
            className={cn(
              "absolute right-0 w-1 cursor-ns-resize touch-none rounded-full bg-muted-foreground/55 transition-opacity duration-150",
              scrollbar.active || alwaysVisible
                ? "pointer-events-auto opacity-100"
                : "pointer-events-none opacity-0",
            )}
            style={{
              height: `${scrollbar.thumbHeight}px`,
              transform: `translateY(${scrollbar.thumbOffset}px)`,
            }}
            onPointerDown={handleThumbPointerDown}
            onPointerMove={handleThumbPointerMove}
            onPointerUp={handleThumbPointerUp}
            onPointerCancel={handleThumbPointerUp}
          />
        </div>
      ) : null}
    </div>
  );
}
