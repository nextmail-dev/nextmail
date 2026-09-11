import { Virtualizer } from "@tanstack/react-virtual";
import { describe, expect, it } from "vitest";
import { pruneMessageMeasurements } from "./message-virtualizer-cache";

describe("message measurements during sustained sync", () => {
  it("keeps a rotating 50-message page bounded over 20,000 arrivals", () => {
    const virtualizer = new Virtualizer<HTMLDivElement, Element>({
      count: 50, getScrollElement: () => null, estimateSize: () => 88,
      scrollToFn: () => undefined, observeElementRect: () => undefined,
      observeElementOffset: () => undefined,
    });
    let largest = 0;
    for (let arrival = 0; arrival < 20_000; arrival++) {
      const keys = Array.from({ length: 50 }, (_, index) => `mailbox:${arrival + index}`);
      // Changing key identity must invalidate measurements even at constant count.
      virtualizer.setOptions({ ...virtualizer.options, getItemKey: (index) => keys[index] });
      virtualizer.getTotalSize();
      virtualizer.resizeItem(0, 90);
      pruneMessageMeasurements(virtualizer, new Set(keys));
      largest = Math.max(largest, virtualizer.itemSizeCache.size);
    }
    expect(largest).toBeLessThanOrEqual(50);
    expect(virtualizer.itemSizeCache.has("mailbox:0")).toBe(false);
    expect(virtualizer.measurementsCache[0].key).toBe("mailbox:19999");
  });
});
