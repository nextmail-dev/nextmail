import { describe, expect, it } from "vitest";

import { formatAddresses, parseAddresses } from "./recipient-utils";

describe("composer recipient fields", () => {
  it("accepts comma and semicolon separated addresses with unicode display names", () => {
    const parsed = parseAddresses("张三 <zhang@example.com>; plain@example.com");
    expect(parsed).toEqual([
      { name: "张三", email: "zhang@example.com" },
      { name: null, email: "plain@example.com" },
    ]);
    expect(formatAddresses(parsed)).toBe(
      "张三 <zhang@example.com>, plain@example.com",
    );
  });
});
