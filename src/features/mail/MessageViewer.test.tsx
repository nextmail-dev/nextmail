import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { MessageViewer } from "./MessageViewer";
import { messageQueryKeys } from "./mail-query-keys";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("@/app/api", () => ({
  api: {
    getMessageDetail: vi.fn().mockResolvedValue({
      id: "message-one",
      mailboxId: "inbox",
      subject: "Attachment",
      from: [{ contactId: null, name: "Alice", headerName: "Alice", email: "alice@example.com" }],
      to: [{ contactId: null, name: null, headerName: null, email: "user@example.com" }],
      cc: [],
      receivedAt: 1,
      plainText: "Please see the attachment.",
      safeHtml: null,
      bodyAvailability: "available",
      attachments: [{
        id: "attachment-one",
        fileName: "report.pdf",
        contentType: "application/pdf",
        size: 2048,
        availability: "missing",
      }],
      remoteImagesBlocked: false,
      revision: 1,
      unread: false,
      flagged: false,
      pendingOperation: false,
    }),
    getReadingPreferences: vi.fn().mockResolvedValue({
      autoLoadRemoteImages: false,
      autoLoadMoreMessages: true,
      autoLoadMoreContacts: true,
    }),
    requestAttachment: vi.fn().mockResolvedValue({
      id: "attachment-one",
      fileName: "report.pdf",
      contentType: "application/pdf",
      size: 2048,
      availability: "available",
    }),
    requestMessageBody: vi.fn(),
    openMessageAttachment: vi.fn().mockResolvedValue(undefined),
    revealMessageAttachment: vi.fn().mockResolvedValue(undefined),
    openRawMessageWindow: vi.fn().mockResolvedValue(undefined),
    openMessagePreviewWindow: vi.fn().mockResolvedValue(undefined),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("MessageViewer", () => {
  it("shows only a spinner while the message body is loading", async () => {
    vi.mocked(api.getMessageDetail).mockResolvedValueOnce({
      id: "message-loading",
      mailboxId: "inbox",
      subject: "Loading body",
      from: [{ contactId: null, name: "Alice", headerName: "Alice", email: "alice@example.com" }],
      to: [{ contactId: null, name: null, headerName: null, email: "user@example.com" }],
      cc: [],
      receivedAt: 1,
      plainText: null,
      safeHtml: null,
      bodyAvailability: "missing",
      attachments: [],
      remoteImagesBlocked: false,
      revision: 1,
      unread: false,
      flagged: false,
      pendingOperation: false,
    });
    vi.mocked(api.requestMessageBody).mockImplementationOnce(() => new Promise(() => undefined));
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const { container } = render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-loading"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    await screen.findByRole("heading", { name: "Loading body" });
    expect(container.querySelector(".animate-spin")).not.toBeNull();
    expect(screen.queryByText("The message body has not been downloaded")).not.toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("keeps the message subject and addressing selectable", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-one"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "Attachment" })).toHaveClass("select-text");
    const sender = screen.getByLabelText("Alice <alice@example.com>");
    const senderName = screen.getByText("Alice");
    expect(sender).not.toContainElement(senderName);
    expect(sender).toHaveTextContent("alice@example.com");
    expect(sender).toHaveClass("bg-muted/55");
    const subject = screen.getByRole("heading", { level: 1 });
    expect(subject).toHaveClass("text-lg", "lg:text-lg");
    const senderHeader = senderName.closest(".border-b");
    expect(senderHeader).toContainElement(subject);
    expect(senderHeader).toContainElement(screen.getByRole("toolbar", { name: "Message actions" }));
    const readerSurface = senderHeader?.parentElement;
    expect(readerSurface).toHaveClass("flex-1", "overflow-hidden", "bg-card");
    expect(readerSurface).not.toHaveClass("mx-5", "mb-5", "rounded-xl", "border");
    const recipient = screen.getByLabelText("user@example.com");
    expect(recipient.closest(".select-text")).not.toBeNull();
    expect(recipient).toHaveClass("bg-muted/55");
  });

  it("keeps HTML mail inset from the reading pane boundaries", async () => {
    vi.mocked(api.getMessageDetail).mockResolvedValueOnce({
      id: "message-html",
      mailboxId: "inbox",
      subject: "HTML message",
      from: [{ contactId: null, name: "Alice", headerName: "Alice", email: "alice@example.com" }],
      to: [{ contactId: null, name: null, headerName: null, email: "user@example.com" }],
      cc: [],
      receivedAt: 1,
      plainText: null,
      safeHtml: "<!doctype html><html><body>HTML body</body></html>",
      bodyAvailability: "available",
      attachments: [],
      remoteImagesBlocked: false,
      revision: 1,
      unread: false,
      flagged: false,
      pendingOperation: false,
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-html"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const frame = await screen.findByTitle("HTML message");
    expect(frame.parentElement).toHaveClass("min-h-0", "flex-1", "overflow-hidden", "px-4", "py-3");
  });

  it("collapses overflowing recipients to one row until explicitly expanded", async () => {
    const scrollHeight = vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(56);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-one"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const toggle = await screen.findByRole("button", { name: "Show recipients" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(toggle);
    expect(screen.getByRole("button", { name: "Hide recipients" })).toHaveAttribute("aria-expanded", "true");
    scrollHeight.mockRestore();
  });

  it("invalidates the exact detail query after an attachment download", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const invalidateQueries = vi.spyOn(queryClient, "invalidateQueries");
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-one"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Open report.pdf" }));

    await waitFor(() => {
      expect(api.requestAttachment).toHaveBeenCalledWith("account-one", "attachment-one");
      expect(api.openMessageAttachment).toHaveBeenCalledWith("account-one", "attachment-one");
      expect(invalidateQueries).toHaveBeenCalledWith({
        queryKey: messageQueryKeys.detail("account-one", "inbox", "message-one"),
      });
    });
  });

  it("opens message source in the independent raw-message window", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-one"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.pointerDown(await screen.findByRole("button", { name: "More actions" }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "View message source" }));

    await waitFor(() => {
      expect(api.openRawMessageWindow).toHaveBeenCalledWith("account-one", "message-one");
    });
    expect(screen.queryByRole("dialog", { name: "Message source" })).not.toBeInTheDocument();
  });

  it("keeps move and copy destinations in the overflow menu", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-one"
          mailboxes={[{
            id: "archive",
            accountId: "account-one",
            name: "Archive",
            delimiter: "/",
            role: "archive",
            selectable: true,
            totalCount: 1,
            unreadCount: 0,
            revision: 1,
          }]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    await screen.findByRole("heading", { name: "Attachment" });
    expect(screen.queryByRole("button", { name: "Move to" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Copy to" })).not.toBeInTheDocument();
    fireEvent.pointerDown(screen.getByRole("button", { name: "More actions" }), {
      button: 0,
      ctrlKey: false,
    });
    expect(await screen.findByRole("menuitem", { name: "Move to" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Copy to" })).toBeInTheDocument();
  });

  it("reveals an attachment through the account-scoped command", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="inbox"
          messageId="message-one"
          mailboxes={[]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const attachment = await screen.findByRole("button", { name: "Open report.pdf" });
    fireEvent.contextMenu(attachment);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show in folder" }));
    await waitFor(() => expect(api.revealMessageAttachment).toHaveBeenCalledWith(
      "account-one",
      "attachment-one",
    ));
  });

  it("hides reply and forward controls for a Drafts message", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <MessageViewer
          accountId="account-one"
          mailboxId="drafts"
          messageId="message-one"
          mailboxes={[{
            id: "drafts",
            accountId: "account-one",
            name: "Drafts",
            delimiter: "/",
            role: "drafts",
            selectable: true,
            totalCount: 1,
            unreadCount: 0,
            revision: 1,
          }]}
          onMessageRemoved={vi.fn()}
        />
      </QueryClientProvider>,
    );

    await screen.findByRole("heading", { name: "Attachment" });
    expect(screen.queryByRole("button", { name: "Reply" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reply all" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Forward" })).not.toBeInTheDocument();
  });
});
