import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { MessageViewer } from "./MessageViewer";
import { messageQueryKeys } from "./mail-query-keys";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }));

vi.mock("@/app/api", () => ({
  api: {
    getMessageDetail: vi.fn().mockResolvedValue({
      id: "message-one",
      mailboxId: "inbox",
      subject: "Attachment",
      from: [{ name: "Alice", email: "alice@example.com" }],
      to: [{ name: null, email: "user@example.com" }],
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
      autoOpenDownloadedAttachments: false,
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

afterEach(cleanup);

describe("MessageViewer", () => {
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
    const recipient = screen.getByLabelText("user@example.com");
    expect(recipient.closest(".select-text")).not.toBeNull();
    expect(recipient).toHaveClass("bg-muted/55");
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

    fireEvent.click(await screen.findByRole("button", { name: "Download report.pdf" }));

    await waitFor(() => {
      expect(api.requestAttachment).toHaveBeenCalledWith("account-one", "attachment-one");
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
