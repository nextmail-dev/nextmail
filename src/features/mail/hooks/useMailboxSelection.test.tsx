import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import type { AccountSummary } from "@/app/types";
import { UNREAD_MAILBOX_ID } from "../mail-query-keys";
import { useMailboxSelection } from "./useMailboxSelection";

vi.mock("@/app/api", () => ({
  api: {
    listMailboxes: vi.fn(),
    setLastSelectedAccount: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));

const accounts: AccountSummary[] = [
  { id: "account-one", email: "alice@example.com", displayName: "Alice" },
  { id: "account-two", email: "bob@example.com", displayName: "Bob" },
];

function createWrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.listMailboxes).mockImplementation(async (accountId) => (
    accountId === "account-one"
      ? [{
        id: "inbox-one",
        accountId: "account-one",
        name: "INBOX",
        role: "inbox",
        selectable: true,
        totalCount: 2,
        unreadCount: 1,
        delimiter: "/",
        revision: 1,
      }]
      : [{
        id: "archive-two",
        accountId: "account-two",
        name: "Archive",
        role: "archive",
        selectable: true,
        totalCount: 1,
        unreadCount: 0,
        delimiter: "/",
        revision: 1,
      }]
  ));
  vi.mocked(api.setLastSelectedAccount).mockImplementation(async (accountId) => accountId);
});

describe("useMailboxSelection", () => {
  it("restores the last account and clears mailbox-local selection when accounts change", async () => {
    const onError = vi.fn();
    const { result } = renderHook(() => useMailboxSelection({
      accounts,
      lastSelectedAccountId: "account-two",
      onError,
    }), { wrapper: createWrapper() });

    expect(result.current.selectedAccountId).toBe("account-two");
    await waitFor(() => expect(result.current.selectedMailboxId).toBe(UNREAD_MAILBOX_ID));
    act(() => {
      result.current.setSelectedMessageId("message-two");
      result.current.setSearchQuery("quarterly");
      result.current.setSubmittedSearchQuery("quarterly");
      result.current.selectAccount("account-one");
    });

    await waitFor(() => {
      expect(result.current.selectedAccountId).toBe("account-one");
      expect(result.current.selectedMailboxId).toBe(UNREAD_MAILBOX_ID);
      expect(result.current.selectedMessageId).toBe("");
      expect(result.current.searchQuery).toBe("");
      expect(result.current.submittedSearchQuery).toBe("");
    });
    expect(api.setLastSelectedAccount).toHaveBeenCalledWith("account-one");
    expect(onError).not.toHaveBeenCalled();
  });

  it("navigates notification targets across accounts and falls back when a mailbox disappeared", async () => {
    const { result } = renderHook(() => useMailboxSelection({
      accounts,
      lastSelectedAccountId: "account-one",
      onError: vi.fn(),
    }), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.selectedMailboxId).toBe(UNREAD_MAILBOX_ID));

    act(() => result.current.navigateToMailLocation({
      accountId: "account-two",
      mailboxId: "archive-two",
      messageId: "message-two",
    }));
    await waitFor(() => {
      expect(result.current.selectedAccountId).toBe("account-two");
      expect(result.current.selectedMailboxId).toBe("archive-two");
      expect(result.current.selectedMessageId).toBe("message-two");
    });

    act(() => result.current.navigateToMailLocation({
      accountId: "account-two",
      mailboxId: "missing-mailbox",
      messageId: "missing-message",
    }));
    await waitFor(() => {
      expect(result.current.selectedMailboxId).toBe("archive-two");
      expect(result.current.selectedMessageId).toBe("");
    });
  });

  it("keeps the current message when the selected mailbox is clicked again", async () => {
    const { result } = renderHook(() => useMailboxSelection({
      accounts,
      lastSelectedAccountId: "account-one",
      onError: vi.fn(),
    }), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.selectedMailboxId).toBe(UNREAD_MAILBOX_ID));

    act(() => result.current.selectMailbox("inbox-one"));
    act(() => {
      result.current.setSelectedMessageId("message-one");
      result.current.setSearchQuery("quarterly");
      result.current.setSubmittedSearchQuery("quarterly");
    });
    act(() => result.current.selectMailbox("inbox-one"));

    expect(result.current.selectedMessageId).toBe("message-one");
    expect(result.current.searchQuery).toBe("quarterly");
    expect(result.current.submittedSearchQuery).toBe("quarterly");

    act(() => result.current.selectMailbox(UNREAD_MAILBOX_ID));
    expect(result.current.selectedMessageId).toBe("");
    expect(result.current.searchQuery).toBe("");
    expect(result.current.submittedSearchQuery).toBe("");
  });
});
