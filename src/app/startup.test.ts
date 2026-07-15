import { describe, expect, it, vi } from "vitest";

import { afterFirstPaint } from "./startup";

describe("afterFirstPaint", () => {
  it("waits until a complete frame has been presented", () => {
    const frames = new Map<number, FrameRequestCallback>();
    let nextHandle = 1;
    const callback = vi.fn();
    const scheduler = {
      requestAnimationFrame(frame: FrameRequestCallback) {
        const handle = nextHandle++;
        frames.set(handle, frame);
        return handle;
      },
      cancelAnimationFrame(handle: number) {
        frames.delete(handle);
      },
    };

    const cancel = afterFirstPaint(callback, scheduler);
    expect(callback).not.toHaveBeenCalled();

    frames.get(1)?.(0);
    expect(callback).not.toHaveBeenCalled();

    frames.get(2)?.(16);
    expect(callback).toHaveBeenCalledOnce();

    cancel();
  });

  it("can be cancelled before background work is scheduled", () => {
    const frames = new Map<number, FrameRequestCallback>();
    const callback = vi.fn();
    const scheduler = {
      requestAnimationFrame(frame: FrameRequestCallback) {
        frames.set(1, frame);
        return 1;
      },
      cancelAnimationFrame(handle: number) {
        frames.delete(handle);
      },
    };

    afterFirstPaint(callback, scheduler)();
    expect(frames.size).toBe(0);
    expect(callback).not.toHaveBeenCalled();
  });
});
