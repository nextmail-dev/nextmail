import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import type { MessageListItem } from "@/app/types";
import i18n from "@/app/i18n";
import { MessageListPane } from "./MessageListPane";

vi.mock("@/app/api", () => ({
  api: {
    listMessages: vi.fn(),
    searchMessages: vi.fn(),
    setMessageRead: vi.fn(),
    setMessageFlagged: vi.fn(),
    openMessageActionComposer: vi.fn(),
    openRemoteDraft: vi.fn(),
    moveMessages: vi.fn(),
    copyMessages: vi.fn(),
    archiveMessages: vi.fn(),
    deleteMessages: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));

const serverResult: MessageListItem = {
  id: "message-one",
  mailboxId: "inbox",
  subject: "Server-side result",
  from: [{ name: "Alice", email: "alice@example.com" }],
  receivedAt: 1,
  preview: "The visible list fields do not contain the query.",
  unread: false,
  flagged: false,
  hasAttachments: true,
  bodyAvailability: "available",
  pendingOperation: false,
};

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.listMessages).mockResolvedValue({ items: [], nextCursor: null });
  vi.mocked(api.searchMessages).mockResolvedValue({
    items: [serverResult],
    nextCursor: null,
  });
});

afterEach(cleanup);

describe("MessageListPane", () => {
  it("uses the debounced server search and keeps results matched by indexed body or attachments", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="inbox"
          mailboxes={[]}
          selectedMessageId=""
          onSelect={vi.fn()}
          onVisibleMessageIdsChange={vi.fn()}
          onMessageRemoved={vi.fn()}
          searchQuery=""
          onSearchChange={vi.fn()}
        />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(api.listMessages).toHaveBeenCalledWith(
      "account-one", "inbox", null, 50,
    ));

    rerender(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="inbox"
          mailboxes={[]}
          selectedMessageId=""
          onSelect={vi.fn()}
          onVisibleMessageIdsChange={vi.fn()}
          onMessageRemoved={vi.fn()}
          searchQuery="annual-report.pdf"
          onSearchChange={vi.fn()}
        />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(api.searchMessages).toHaveBeenCalledWith(
      "account-one", "inbox", "annual-report.pdf", null, 50,
    ));
    expect(await screen.findByText("Server-side result")).toBeInTheDocument();
  });

  it("clears the current selection when the selected row is clicked again", async () => {
    vi.mocked(api.listMessages).mockResolvedValue({ items: [serverResult], nextCursor: null });
    const onSelect = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="inbox"
          mailboxes={[]}
          selectedMessageId="message-one"
          onSelect={onSelect}
          onVisibleMessageIdsChange={vi.fn()}
          onMessageRemoved={vi.fn()}
          searchQuery=""
          onSearchChange={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: /Alice.*Server-side result/i }));
    expect(onSelect).toHaveBeenCalledWith("");
  });
});
