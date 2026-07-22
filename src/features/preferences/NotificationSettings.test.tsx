import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import type { MailboxSummary, NotificationPreferences } from "@/app/types";
import {
  NotificationSettings,
  notificationAccountEnabled,
  notificationFolderEnabled,
} from "./NotificationSettings";

const initialPreferences: NotificationPreferences = {
  enabled: true,
  displayMode: "stacked",
  maxStacked: 3,
  displayDurationSeconds: 5,
  accounts: [],
  folders: [],
};

const inbox: MailboxSummary = {
  id: "inbox",
  accountId: "account-one",
  name: "Inbox",
  delimiter: "/",
  role: "inbox",
  selectable: true,
  totalCount: 2,
  unreadCount: 1,
  revision: 1,
};

const archive: MailboxSummary = {
  ...inbox,
  id: "archive",
  name: "Archive",
  role: "archive",
};

vi.mock("@/app/api", () => ({
  api: {
    getNotificationPreferences: vi.fn(),
    setNotificationPreferences: vi.fn(),
    listMailboxes: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({ code: "common.unexpected_error", params: {}, retryable: false })),
}));

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.mocked(api.getNotificationPreferences).mockResolvedValue(initialPreferences);
  vi.mocked(api.setNotificationPreferences).mockImplementation((preferences) => Promise.resolve(preferences));
  vi.mocked(api.listMailboxes).mockResolvedValue([inbox, archive]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("NotificationSettings", () => {
  it("persists global and folder settings with inbox-only defaults", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <NotificationSettings accounts={[{ id: "account-one", email: "alice@example.com", displayName: "Alice" }]} />
      </QueryClientProvider>,
    );

    const globalSwitch = await screen.findByRole("switch", { name: "New mail notifications" });
    expect(globalSwitch).toBeChecked();
    expect(screen.getByRole("switch", { name: "Notifications for Alice" })).toBeChecked();
    fireEvent.click(globalSwitch);
    await waitFor(() => {
      const calls = vi.mocked(api.setNotificationPreferences).mock.calls;
      expect(calls[0]?.[0]).toEqual({
        ...initialPreferences,
        enabled: false,
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "Manage notification folders for Alice" }));
    expect(await screen.findByRole("switch", { name: "Notifications for Inbox" })).toBeChecked();
    const archiveSwitch = screen.getByRole("switch", { name: "Notifications for Archive" });
    expect(archiveSwitch.closest(".overflow-y-scroll")).toHaveClass(
      "is-scrolling",
      "[scrollbar-gutter:stable]",
    );
    expect(archiveSwitch).not.toBeChecked();
    fireEvent.click(archiveSwitch);
    await waitFor(() => {
      const calls = vi.mocked(api.setNotificationPreferences).mock.calls;
      expect(calls[calls.length - 1]?.[0].folders).toEqual([
        { accountId: "account-one", mailboxId: "archive", enabled: true },
      ]);
    });
  });

  it("resolves explicit account and folder overrides", () => {
    const preferences: NotificationPreferences = {
      ...initialPreferences,
      accounts: [{ accountId: "account-one", enabled: false }],
      folders: [{ accountId: "account-one", mailboxId: "archive", enabled: true }],
    };
    expect(notificationAccountEnabled(preferences, "account-one")).toBe(false);
    expect(notificationAccountEnabled(preferences, "other")).toBe(true);
    expect(notificationFolderEnabled(preferences, "account-one", inbox)).toBe(true);
    expect(notificationFolderEnabled(preferences, "account-one", archive)).toBe(true);
    expect(notificationFolderEnabled(initialPreferences, "account-one", archive)).toBe(false);
  });
});
