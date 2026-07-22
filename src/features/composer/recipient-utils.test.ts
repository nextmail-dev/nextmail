import { describe, expect, it } from "vitest";

import { addRecipientInput, formatAddresses, isValidEmailAddress, parseAddresses } from "./recipient-utils";

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

  it("validates practical mailbox syntax and reports the first invalid token", () => {
    expect(isValidEmailAddress("reader@example.com")).toBe(true);
    expect(isValidEmailAddress("missing-at.example.com")).toBe(false);
    expect(addRecipientInput([], "reader@example.com; broken-address")).toEqual({
      addresses: [],
      invalid: "broken-address",
    });
  });

  it("deduplicates committed addresses case-insensitively", () => {
    expect(addRecipientInput(
      [{ name: null, email: "reader@example.com" }],
      "Reader <READER@example.com>, second@example.com",
    )).toEqual({
      addresses: [
        { name: null, email: "reader@example.com" },
        { name: null, email: "second@example.com" },
      ],
      invalid: null,
    });
  });
});
