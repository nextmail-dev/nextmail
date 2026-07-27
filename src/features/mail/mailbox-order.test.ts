import { describe, expect, it } from "vitest";

import type { MailboxSummary } from "@/app/types";
import { flattenMailboxHierarchy } from "./MailboxPane";
import { reorderMailboxHierarchy } from "./mailbox-order";

function mailbox(id: string, name: string): MailboxSummary {
  return {
    id,
    accountId: "account-one",
    name,
    delimiter: "/",
    role: "other",
    selectable: true,
    totalCount: 0,
    unreadCount: 0,
    revision: 1,
  };
}

describe("reorderMailboxHierarchy", () => {
  it("moves a parent and its complete subtree as one local-order block", () => {
    const items = flattenMailboxHierarchy([
      mailbox("projects", "Projects"),
      mailbox("projects-2026", "Projects/2026"),
      mailbox("archive", "Archive"),
      mailbox("archive-2025", "Archive/2025"),
    ]);

    expect(reorderMailboxHierarchy(items, "projects", "archive", "after")).toEqual([
      "archive",
      "archive-2025",
      "projects",
      "projects-2026",
    ]);
  });

  it("rejects cross-parent drops so drag sorting cannot mutate server hierarchy", () => {
    const items = flattenMailboxHierarchy([
      mailbox("projects", "Projects"),
      mailbox("projects-2026", "Projects/2026"),
      mailbox("archive", "Archive"),
    ]);

    expect(reorderMailboxHierarchy(
      items,
      "projects-2026",
      "archive",
      "before",
    )).toBeNull();
  });

  it("places same-level folders before or after the target", () => {
    const items = flattenMailboxHierarchy([
      mailbox("one", "One"),
      mailbox("two", "Two"),
      mailbox("three", "Three"),
    ]);

    expect(reorderMailboxHierarchy(items, "three", "one", "before")).toEqual([
      "three",
      "one",
      "two",
    ]);
    expect(reorderMailboxHierarchy(items, "one", "two", "after")).toEqual([
      "two",
      "one",
      "three",
    ]);
  });
});
