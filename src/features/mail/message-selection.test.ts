import { describe, expect, it } from "vitest";

import { nextMessageIdAfterRemoval, nextMessageIdAfterRemovals } from "./message-selection";

describe("nextMessageIdAfterRemoval", () => {
  it("prefers the following message in the current visible order", () => {
    expect(nextMessageIdAfterRemoval(["one", "two", "three"], "two")).toBe("three");
  });

  it("falls back to the previous message when the removed message was last", () => {
    expect(nextMessageIdAfterRemoval(["one", "two"], "two")).toBe("one");
  });

  it("clears selection when no neighbor exists", () => {
    expect(nextMessageIdAfterRemoval(["one"], "one")).toBe("");
    expect(nextMessageIdAfterRemoval(["one"], "missing")).toBe("");
  });
});

describe("nextMessageIdAfterRemovals", () => {
  it("skips every removed neighbor after a bulk operation", () => {
    expect(nextMessageIdAfterRemovals(
      ["one", "two", "three", "four"],
      ["two", "three"],
      "two",
    )).toBe("four");
  });

  it("keeps a selected message that was not removed", () => {
    expect(nextMessageIdAfterRemovals(["one", "two"], ["two"], "one")).toBe("one");
  });
});
