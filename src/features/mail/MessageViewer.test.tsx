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
    }),
    requestAttachment: vi.fn().mockResolvedValue({
      id: "attachment-one",
      fileName: "report.pdf",
      contentType: "application/pdf",
      size: 2048,
      availability: "available",
    }),
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
    expect(screen.getByText("Alice")).toHaveClass("select-text");
    expect(screen.getByText("alice@example.com")).toHaveClass("select-text");
    expect(screen.getByText(/user@example\.com/)).toHaveClass("select-text");
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
});
