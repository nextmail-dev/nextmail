import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import type { MessageListItem } from "@/app/types";
import i18n from "@/app/i18n";
import { MessageListPane } from "./MessageListPane";

vi.mock("@/app/api", () => ({
  api: {
    listMessages: vi.fn(),
    searchMessages: vi.fn(),
    getReadingPreferences: vi.fn(),
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
  vi.mocked(api.getReadingPreferences).mockResolvedValue({
    autoLoadRemoteImages: false,
    autoOpenDownloadedAttachments: true,
    autoLoadMoreMessages: true,
  });
  vi.mocked(api.searchMessages).mockResolvedValue({
    items: [serverResult],
    nextCursor: null,
  });
});

afterEach(cleanup);

describe("MessageListPane", () => {
  it("waits for the explicit search button before querying indexed body or attachments", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <ControlledSearchPane />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(api.listMessages).toHaveBeenCalledWith(
      "account-one", "inbox", null, 50,
    ));

    fireEvent.change(screen.getByRole("searchbox", { name: "Search this folder" }), {
      target: { value: "annual-report.pdf" },
    });
    await new Promise((resolve) => window.setTimeout(resolve, 300));
    expect(api.searchMessages).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Search this folder" }));

    await waitFor(() => expect(api.searchMessages).toHaveBeenCalledWith(
      "account-one", "inbox", "annual-report.pdf", null, 50,
    ));
    expect(await screen.findByText("Server-side result")).toBeInTheDocument();
  });

  it("submits the current search when the search form is submitted from the keyboard", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <ControlledSearchPane />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(api.listMessages).toHaveBeenCalled());

    const searchbox = screen.getByRole("searchbox", { name: "Search this folder" });
    fireEvent.change(searchbox, { target: { value: "quarterly" } });
    fireEvent.submit(searchbox.closest("form")!);

    await waitFor(() => expect(api.searchMessages).toHaveBeenCalledWith(
      "account-one", "inbox", "quarterly", null, 50,
    ));
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
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: /Alice.*Server-side result/i }));
    expect(onSelect).toHaveBeenCalledWith("");
  });

  it("loads the next page automatically near the bottom when the reading preference is enabled", async () => {
    vi.mocked(api.listMessages)
      .mockResolvedValueOnce({ items: [serverResult], nextCursor: "next-page" })
      .mockResolvedValueOnce({
        items: [{ ...serverResult, id: "message-two", subject: "Second page" }],
        nextCursor: null,
      });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const { container } = render(
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
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    await screen.findByText("Server-side result");
    const viewport = container.querySelector(".native-scrollbar-hidden") as HTMLDivElement;
    Object.defineProperties(viewport, {
      clientHeight: { configurable: true, value: 400 },
      scrollHeight: { configurable: true, value: 900 },
      scrollTop: { configurable: true, value: 380, writable: true },
    });
    fireEvent.scroll(viewport);

    await waitFor(() => expect(api.listMessages).toHaveBeenCalledWith(
      "account-one", "inbox", "next-page", 50,
    ));
    expect(await screen.findByText("Second page")).toBeInTheDocument();
  });
});

function ControlledSearchPane() {
  const [searchQuery, setSearchQuery] = useState("");
  const [submittedSearchQuery, setSubmittedSearchQuery] = useState("");
  return (
    <MessageListPane
      accountId="account-one"
      mailboxId="inbox"
      mailboxes={[]}
      selectedMessageId=""
      onSelect={vi.fn()}
      onVisibleMessageIdsChange={vi.fn()}
      onMessageRemoved={vi.fn()}
      searchQuery={searchQuery}
      submittedSearchQuery={submittedSearchQuery}
      onSearchChange={setSearchQuery}
      onSearchSubmit={setSubmittedSearchQuery}
    />
  );
}
