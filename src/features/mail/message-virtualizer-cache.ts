import type { Virtualizer } from "@tanstack/react-virtual";

// Virtual-core retains measured sizes after their message leaves the query.
// During first sync a 50-item page can cycle through tens of thousands of IDs.
export function pruneMessageMeasurements(
  virtualizer: Virtualizer<HTMLDivElement, Element>,
  keys: ReadonlySet<string>,
) {
  for (const key of virtualizer.itemSizeCache.keys()) {
    if (!keys.has(String(key))) virtualizer.itemSizeCache.delete(key);
  }
  // Use the library's cleanup so detached elements are also unobserved.
  virtualizer.measureElement(null);
}
