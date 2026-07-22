import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import type { AccountSummary } from "@/app/types";
import { AccountManagementPanel, AccountsManagement, nextAccountIdAfterRemoval } from "./AccountManagementDialog";

vi.mock("@/app/api", () => ({
  api: {
    getAccountManagementDetail: vi.fn(),
    getAccountConnectionDraft: vi.fn(),
    getSyncProgress: vi.fn(),
    listAccountRuntimeSummaries: vi.fn(),
    listMailboxes: vi.fn(),
    getAccountRemovalImpact: vi.fn(),
    setAccountSyncPolicy: vi.fn(),
    setAccountSyncInterval: vi.fn(),
    setDownloadNonInboxBodies: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({ code: "common.unexpected_error", params: {}, retryable: false })),
}));

const accounts: AccountSummary[] = [
  { id: "account-one", email: "alice@example.com", displayName: "Alice" },
  { id: "account-two", email: "bob@example.com", displayName: "Bob" },
];

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.mocked(api.getAccountManagementDetail).mockResolvedValue({
    id: "account-one",
    email: "alice@example.com",
    displayName: "Alice",
    incomingHost: "imap.example.com",
    incomingPort: 993,
    security: "tls",
    syncPolicy: "days90",
    syncInterval: "minutes1",
    downloadNonInboxBodies: false,
  });
  vi.mocked(api.getSyncProgress).mockResolvedValue({
    accountId: "account-one",
    phase: "complete",
    completed: 1,
    total: 1,
    currentMailboxName: null,
    errorCode: null,
    revision: 1,
  });
  vi.mocked(api.listAccountRuntimeSummaries).mockResolvedValue([
    { accountId: "account-one", state: "ready", errorCode: null, retryAt: null, revision: 1 },
  ]);
  vi.mocked(api.listMailboxes).mockResolvedValue([]);
});

afterEach(cleanup);

describe("AccountsManagement", () => {
  it("reuses the account list and changes the managed account", () => {
    const onSelectedAccountChange = vi.fn();
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <AccountsManagement
          accounts={accounts}
          selectedAccountId="account-one"
          onSelectedAccountChange={onSelectedAccountChange}
          enabled={false}
        />
      </QueryClientProvider>,
    );

    expect(screen.getByText("Email accounts")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add account" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Bob bob@example\.com/ }));
    expect(onSelectedAccountChange).toHaveBeenCalledWith("account-two");
  });

  it("chooses another account after removal and clears the last account", () => {
    expect(nextAccountIdAfterRemoval(accounts, "account-one")).toBe("account-two");
    expect(nextAccountIdAfterRemoval([accounts[0]], "account-one")).toBe("");
  });

  it("keeps a formal add-account entry when no accounts remain", () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <AccountsManagement
          accounts={[]}
          selectedAccountId=""
          onSelectedAccountChange={vi.fn()}
          enabled={false}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Add account" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("Add an email account");
  });

  it("shows the persisted per-account automatic synchronization interval", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <AccountManagementPanel accountId="account-one" />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Automatic synchronization")).toBeInTheDocument();
    expect(screen.getByText("Every minute")).toBeInTheDocument();
  });
});
