import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { SettingsApp } from "./SettingsApp";

vi.mock("@/app/api", () => ({
  api: {
    getPreferences: vi.fn().mockResolvedValue({
      theme: "system",
      accentColor: "#2563eb",
      language: "en-US",
    }),
    listAccountSummaries: vi.fn().mockResolvedValue([
      { id: "account-one", email: "alice@example.com", displayName: "Alice" },
      { id: "account-two", email: "bob@example.com", displayName: "Bob" },
    ]),
    getAccountManagementDetail: vi.fn().mockResolvedValue({
      id: "account-one",
      email: "alice@example.com",
      displayName: "Alice",
      incomingHost: "imap.example.com",
      incomingPort: 993,
      security: "tls",
      syncPolicy: "days90",
    }),
    listMailboxes: vi.fn().mockResolvedValue([]),
    listAccountRuntimeSummaries: vi.fn().mockResolvedValue([
      { accountId: "account-one", state: "ready", errorCode: null, retryAt: null, revision: 1 },
    ]),
    getSyncProgress: vi.fn().mockResolvedValue({
      accountId: "account-one",
      phase: "complete",
      completed: 1,
      total: 1,
      errorCode: null,
      revision: 1,
    }),
    getAccountRemovalImpact: vi.fn().mockResolvedValue({
      editingDrafts: 0,
      queuedSendJobs: 0,
      pendingOperations: 0,
      canRemove: true,
    }),
    getAppAbout: vi.fn().mockResolvedValue({ name: "NextMail", version: "0.1.0" }),
    getReadingPreferences: vi.fn().mockResolvedValue({
      autoLoadRemoteImages: false,
      autoOpenDownloadedAttachments: true,
    }),
    setAppearancePreferences: vi.fn().mockImplementation((preferences) => Promise.resolve(preferences)),
    setReadingPreferences: vi.fn().mockImplementation((preferences) => Promise.resolve(preferences)),
    getNotificationPreferences: vi.fn().mockResolvedValue({
      enabled: true,
      displayMode: "stacked",
      maxStacked: 3,
      displayDurationSeconds: 5,
      accounts: [],
      folders: [],
    }),
    setNotificationPreferences: vi.fn().mockImplementation((preferences) => Promise.resolve(preferences)),
  },
  normalizeCommandError: vi.fn(() => ({ code: "common.unexpected_error", params: {}, retryable: false })),
}));

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(cleanup);

describe("SettingsApp", () => {
  it("renders the independent settings shell after loading preferences and accounts", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "General" })).toBeInTheDocument();
    expect(screen.getByText("Language")).toBeInTheDocument();
    expect(document.querySelector(".native-scrollbar-hidden")).toBeInTheDocument();
  });

  it("exposes the persisted remote-image preference in Reading", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Reading" }));
    expect(screen.getByText("Automatically load remote images")).toBeInTheDocument();
    expect(screen.getAllByRole("checkbox")[0]).not.toBeChecked();
    expect(screen.getByText("Open attachments after downloading")).toBeInTheDocument();
    expect(screen.getAllByRole("checkbox")[1]).toBeChecked();
  });

  it("offers an accessible theme-color palette instead of a select", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Appearance" }));
    expect(screen.getByText("Theme color")).toBeInTheDocument();
    expect(screen.getAllByRole("radio")).toHaveLength(10);
    expect(screen.getByRole("radio", { name: "Blue" })).toBeChecked();

    fireEvent.click(screen.getByRole("radio", { name: "Orange" }));
    await waitFor(() => {
      const calls = vi.mocked(api.setAppearancePreferences).mock.calls;
      expect(calls[calls.length - 1]?.[0]).toEqual({
        theme: "system",
        accentColor: "#ea580c",
        language: "en-US",
      });
    });
  });

  it("does not duplicate account management in the settings categories", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Accounts" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Composing" })).toBeInTheDocument();
  });

  it("renders notification preferences instead of a placeholder", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Notifications" }));
    expect(await screen.findByRole("switch", { name: "New mail notifications" })).toBeChecked();
    expect(screen.queryByText("No adjustable options yet")).not.toBeInTheDocument();
  });
});
