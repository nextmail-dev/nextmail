import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import type { MessageListItem } from "@/app/types";
import i18n from "@/app/i18n";
import { MessageListPane } from "./MessageListPane";
import { STARRED_MAILBOX_ID, UNREAD_MAILBOX_ID } from "./mail-query-keys";

vi.mock("@/app/api", () => ({
  api: {
    listMessages: vi.fn(),
    listUnreadMessages: vi.fn(),
    listStarredMessages: vi.fn(),
    searchMessages: vi.fn(),
    getReadingPreferences: vi.fn(),
    setMessageRead: vi.fn(),
    setMessageFlagged: vi.fn(),
    openMessagePreviewWindow: vi.fn(),
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
  from: [{ contactId: null, name: "Alice", headerName: "Alice", email: "alice@example.com" }],
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
  vi.mocked(api.listUnreadMessages).mockResolvedValue({ items: [], nextCursor: null });
  vi.mocked(api.listStarredMessages).mockResolvedValue({ items: [], nextCursor: null });
  vi.mocked(api.getReadingPreferences).mockResolvedValue({
    autoLoadRemoteImages: false,
    autoLoadMoreMessages: true,
    autoLoadMoreContacts: true,
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

  it("clears the current row when it is clicked again", async () => {
    vi.mocked(api.listMessages).mockResolvedValue({
      items: [serverResult, { ...serverResult, id: "message-two", subject: "Second message", unread: true }],
      nextCursor: null,
    });
    const onSelect = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const { container } = render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="inbox"
          mailboxes={[]}
          selectedMessageId="message-one"
          onSelect={onSelect}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const rowButton = await screen.findByRole("button", { name: /Alice.*Server-side result/i });
    const row = rowButton.parentElement;
    const lastRow = screen.getByRole("button", { name: /Alice.*Second message/i }).parentElement;
    const viewport = container.querySelector(".native-scrollbar-hidden");
    const scrollArea = container.querySelector('[data-scrollbar-auto-hide="true"]');
    expect(scrollArea).toBeInTheDocument();
    expect(viewport).not.toHaveClass("pr-4");
    expect(row).toHaveClass(
      "bg-selection",
      "before:w-0.5",
      "after:inset-x-5",
      "after:h-px",
      "after:bg-border/80",
    );
    expect(rowButton).toHaveClass("py-2.5", "pr-12", "pl-10");
    expect(row).not.toHaveClass("after:hidden");
    expect(lastRow).toHaveClass("after:hidden", "bg-primary/[0.035]");
    expect(screen.getByText("Second message")).toHaveClass("font-semibold");

    fireEvent.click(rowButton);
    expect(onSelect).toHaveBeenCalledWith("", "");
    expect(rowButton).toHaveAttribute("aria-pressed", "false");
  });

  it("moves message selection with arrow keys instead of scrolling", async () => {
    vi.mocked(api.listMessages).mockResolvedValue({
      items: [serverResult, { ...serverResult, id: "message-two", subject: "Second message" }],
      nextCursor: null,
    });
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
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const first = await screen.findByRole("button", { name: /Alice.*Server-side result/i });
    const second = screen.getByRole("button", { name: /Alice.*Second message/i });
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(onSelect).toHaveBeenCalledWith("message-two", "inbox");
    expect(second).toHaveFocus();
  });

  it("toggles read state from a solid dot or faint hollow ring without selecting the row", async () => {
    vi.mocked(api.listMessages).mockResolvedValue({
      items: [serverResult, { ...serverResult, id: "message-two", subject: "Unread message", unread: true }],
      nextCursor: null,
    });
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
          selectedMessageId=""
          onSelect={onSelect}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const markUnread = await screen.findByRole("button", { name: "Mark as unread" });
    expect(markUnread).toHaveClass("hover:bg-transparent");
    expect(markUnread.querySelector("span")).toHaveClass(
      "border-foreground/15",
      "group-hover/read-state:ring-2",
      "group-hover/read-state:ring-foreground/10",
    );
    fireEvent.click(markUnread);
    await waitFor(() => expect(api.setMessageRead).toHaveBeenCalledWith(
      "account-one", "inbox", ["message-one"], false,
    ));
    expect(onSelect).not.toHaveBeenCalled();

    const markRead = screen.getByRole("button", { name: "Mark as read" });
    expect(markRead.querySelector("span")).toHaveClass("bg-primary");
    fireEvent.click(markRead);
    await waitFor(() => expect(api.setMessageRead).toHaveBeenCalledWith(
      "account-one", "inbox", ["message-two"], true,
    ));
  });

  it("lists account-wide unread messages and keeps their real mailbox location", async () => {
    const unread = { ...serverResult, mailboxId: "archive", unread: true };
    vi.mocked(api.listUnreadMessages).mockResolvedValue({ items: [unread], nextCursor: null });
    const onSelect = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId={UNREAD_MAILBOX_ID}
          mailboxes={[{
            id: "archive",
            accountId: "account-one",
            name: "Archive",
            delimiter: "/",
            role: "archive",
            selectable: true,
            totalCount: 2,
            unreadCount: 1,
            revision: 1,
          }]}
          selectedMessageId=""
          onSelect={onSelect}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Unread")).toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: /Alice.*Server-side result/i }));
    expect(onSelect).toHaveBeenCalledWith("message-one", "archive");
  });

  it("lists account-wide starred messages without folder search", async () => {
    vi.mocked(api.listStarredMessages).mockResolvedValue({
      items: [{ ...serverResult, mailboxId: "archive", flagged: true }],
      nextCursor: null,
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId={STARRED_MAILBOX_ID}
          mailboxes={[]}
          selectedMessageId=""
          onSelect={vi.fn()}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Starred")).toBeInTheDocument();
    expect(api.listStarredMessages).toHaveBeenCalledWith("account-one", null, 50);
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  });

  it("opens a message in an independent window from double click or the context menu", async () => {
    vi.mocked(api.listMessages).mockResolvedValue({ items: [serverResult], nextCursor: null });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="inbox"
          mailboxes={[]}
          selectedMessageId=""
          onSelect={vi.fn()}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const row = await screen.findByRole("button", { name: /Alice.*Server-side result/i });
    fireEvent.doubleClick(row);
    await waitFor(() => expect(api.openMessagePreviewWindow).toHaveBeenCalledWith(
      "account-one", "inbox", "message-one",
    ));

    vi.mocked(api.openMessagePreviewWindow).mockClear();
    fireEvent.contextMenu(row);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Open in new window" }));
    await waitFor(() => expect(api.openMessagePreviewWindow).toHaveBeenCalledWith(
      "account-one", "inbox", "message-one",
    ));
  });

  it("applies a context-menu operation to the current multi-selection", async () => {
    const second = {
      ...serverResult,
      id: "message-two",
      subject: "Second message",
      from: [{ ...serverResult.from[0], name: "Bob", email: "bob@example.com" }],
    };
    const third = {
      ...serverResult,
      id: "message-three",
      subject: "Third message",
      from: [{ ...serverResult.from[0], name: "Carol", email: "carol@example.com" }],
    };
    vi.mocked(api.listMessages).mockResolvedValue({ items: [serverResult, second, third], nextCursor: null });
    const onMessagesRemoved = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="inbox"
          mailboxes={[]}
          selectedMessageId=""
          onSelect={vi.fn()}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={onMessagesRemoved}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const firstRow = await screen.findByRole("button", { name: /Alice.*Server-side result/i });
    const secondRow = await screen.findByRole("button", { name: /Bob.*Second message/i });
    const thirdRow = await screen.findByRole("button", { name: /Carol.*Third message/i });
    fireEvent.click(firstRow);
    fireEvent.click(thirdRow, { shiftKey: true });
    fireEvent.click(secondRow, { ctrlKey: true });
    fireEvent.click(secondRow, { ctrlKey: true });
    fireEvent.contextMenu(secondRow);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));

    await waitFor(() => expect(api.deleteMessages).toHaveBeenCalledWith(
      "account-one",
      "inbox",
      ["message-one", "message-two", "message-three"],
    ));
    expect(onMessagesRemoved).toHaveBeenCalledWith(["message-one", "message-two", "message-three"]);
  });

  it("does not offer reply or forward actions for messages in Drafts", async () => {
    vi.mocked(api.listMessages).mockResolvedValue({ items: [serverResult], nextCursor: null });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane
          accountId="account-one"
          mailboxId="drafts"
          mailbox={{
            id: "drafts",
            accountId: "account-one",
            name: "Drafts",
            delimiter: "/",
            role: "drafts",
            selectable: true,
            totalCount: 1,
            unreadCount: 0,
            revision: 1,
          }}
          mailboxes={[]}
          selectedMessageId=""
          onSelect={vi.fn()}
          onVisibleMessageIdsChange={vi.fn()}
          onMessagesRemoved={vi.fn()}
          searchQuery=""
          submittedSearchQuery=""
          onSearchChange={vi.fn()}
          onSearchSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.contextMenu(await screen.findByRole("button", { name: /Alice.*Server-side result/i }));
    expect(await screen.findByRole("menuitem", { name: "Continue editing draft" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Reply" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Reply all" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Forward" })).not.toBeInTheDocument();
  });

  it("resets the message viewport when the mailbox changes", async () => {
    vi.mocked(api.listMessages).mockImplementation(async (_accountId, mailboxId) => ({
      items: [{
        ...serverResult,
        subject: mailboxId === "archive" ? "Archive result" : serverResult.subject,
      }],
      nextCursor: null,
    }));
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const props = {
      accountId: "account-one",
      mailboxes: [],
      selectedMessageId: "",
      onSelect: vi.fn(),
      onVisibleMessageIdsChange: vi.fn(),
      onMessagesRemoved: vi.fn(),
      searchQuery: "",
      submittedSearchQuery: "",
      onSearchChange: vi.fn(),
      onSearchSubmit: vi.fn(),
    };
    const { container, rerender } = render(
      <QueryClientProvider client={queryClient}>
        <MessageListPane {...props} mailboxId="inbox" />
      </QueryClientProvider>,
    );
    await screen.findByText("Server-side result");
    const inboxViewport = container.querySelector(".native-scrollbar-hidden") as HTMLDivElement;
    inboxViewport.scrollTop = 280;

    rerender(
      <QueryClientProvider client={queryClient}>
        <MessageListPane {...props} mailboxId="archive" />
      </QueryClientProvider>,
    );
    await screen.findByText("Archive result");
    await waitFor(() => expect(api.listMessages).toHaveBeenCalledWith(
      "account-one", "archive", null, 50,
    ));
    const archiveViewport = container.querySelector(".native-scrollbar-hidden") as HTMLDivElement;
    expect(archiveViewport).not.toBe(inboxViewport);
    expect(archiveViewport.scrollTop).toBe(0);
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
          onMessagesRemoved={vi.fn()}
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
      onMessagesRemoved={vi.fn()}
      searchQuery={searchQuery}
      submittedSearchQuery={submittedSearchQuery}
      onSearchChange={setSearchQuery}
      onSearchSubmit={setSubmittedSearchQuery}
    />
  );
}
