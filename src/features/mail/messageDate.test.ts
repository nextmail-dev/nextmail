import { describe, expect, it } from "vitest";

import { formatMessageListTimestamp } from "./messageDate";

describe("formatMessageListTimestamp", () => {
  const now = new Date(2026, 6, 15, 12, 30);
  const seconds = (value: Date) => value.getTime() / 1000;

  it("uses time for today and a localized label for yesterday", () => {
    expect(formatMessageListTimestamp(seconds(new Date(2026, 6, 15, 8, 5)), "昨天", now)).toBe("08:05");
    expect(formatMessageListTimestamp(seconds(new Date(2026, 6, 14, 23, 59)), "昨天", now)).toBe("昨天");
  });

  it("uses month-day within the year and a full date across years", () => {
    expect(formatMessageListTimestamp(seconds(new Date(2026, 0, 9, 8, 5)), "昨天", now)).toBe("01-09");
    expect(formatMessageListTimestamp(seconds(new Date(2025, 11, 31, 8, 5)), "昨天", now)).toBe("2025-12-31");
  });
});
