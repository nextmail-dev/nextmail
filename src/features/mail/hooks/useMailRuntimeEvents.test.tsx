import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { mailQueryKeys, messageQueryKeys } from "../mail-query-keys";
import { useMailRuntimeEvents } from "./useMailRuntimeEvents";

const { listenMock } = vi.hoisted(() => ({ listenMock: vi.fn() }));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

type EventHandler = (event: { payload: never }) => void;

function createWrapper(client: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("useMailRuntimeEvents", () => {
  it("maps runtime events to account-scoped invalidations with stable listeners", async () => {
    const handlers = new Map<string, EventHandler>();
    const disposers: Array<ReturnType<typeof vi.fn>> = [];
    listenMock.mockImplementation((eventName, handler) => {
      handlers.set(eventName, handler);
      const dispose = vi.fn();
      disposers.push(dispose);
      return Promise.resolve(dispose);
    });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const invalidate = vi.spyOn(client, "invalidateQueries");
    const refetch = vi.spyOn(client, "refetchQueries");
    const onSent = vi.fn();
    const onNavigate = vi.fn();
    const { rerender, unmount } = renderHook(
      ({ selectedAccountId, selectedMailboxId }) => useMailRuntimeEvents({ selectedAccountId, selectedMailboxId, onSent, onNavigate }),
      {
        initialProps: { selectedAccountId: "account-one", selectedMailboxId: "inbox" },
        wrapper: createWrapper(client),
      },
    );
    await waitFor(() => expect(handlers.size).toBe(9));

    act(() => handlers.get("mailbox-changed")?.({
      payload: { accountId: "account-two", mailboxId: "archive" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.mailboxes("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.messagesForMailbox("account-two", "archive") });

    let finishFirstRefresh: (() => void) | undefined;
    refetch.mockClear();
    let explicitRefreshCount = 0;
    refetch.mockImplementation((filters) => {
      if (filters?.exact) {
        explicitRefreshCount += 1;
        if (explicitRefreshCount === 1) {
          return new Promise<void>((resolve) => {
            finishFirstRefresh = resolve;
          });
        }
      }
      return Promise.resolve();
    });
    act(() => {
      handlers.get("mailbox-changed")?.({
        payload: { accountId: "account-one", mailboxId: "inbox" } as never,
      });
      handlers.get("mailbox-changed")?.({
        payload: { accountId: "account-one", mailboxId: "inbox" } as never,
      });
    });
    await waitFor(() => expect(refetch.mock.calls.filter(([filters]) => filters?.exact)).toHaveLength(1));
    expect(refetch).toHaveBeenCalledWith({
      queryKey: mailQueryKeys.messagesForMailbox("account-one", "inbox"),
      exact: true,
      type: "active",
    });
    act(() => finishFirstRefresh?.());
    await waitFor(() => expect(refetch.mock.calls.filter(([filters]) => filters?.exact)).toHaveLength(2));

    // message-arrived: incrementally inserts the item into the selected
    // mailbox cache, and invalidates the cache for other mailboxes.
    const setQueryData = vi.spyOn(client, "setQueryData");
    const messagesKey = mailQueryKeys.messagesForMailbox("account-one", "inbox");
    client.setQueryData(messagesKey, { pages: [{ items: [], nextCursor: null }], pageParams: [null] });
    setQueryData.mockClear();
    invalidate.mockClear();
    const arrivedItem = {
      id: "message-arrived",
      mailboxId: "inbox",
      subject: "Hello",
      from: [{ name: "Sender", email: "sender@example.com" }],
      receivedAt: 1000,
      preview: "preview",
      unread: true,
      flagged: false,
      hasAttachments: false,
      bodyAvailability: "missing",
      pendingOperation: false,
    };
    act(() => handlers.get("message-arrived")?.({
      payload: { accountId: "account-one", mailboxId: "inbox", item: arrivedItem } as never,
    }));
    // message-arrived for the selected mailbox is buffered and flushed on a
    // short timer to coalesce rapid arrivals into a single cache update.
    await waitFor(() => expect(setQueryData).toHaveBeenCalledWith(messagesKey, expect.any(Function)));
    await waitFor(() => expect(
      (client.getQueryData(messagesKey) as { pages: { items: unknown[] }[] } | undefined)
        ?.pages[0].items,
    ).toEqual([arrivedItem]));
    act(() => handlers.get("message-arrived")?.({
      payload: { accountId: "account-two", mailboxId: "archive", item: arrivedItem } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: mailQueryKeys.messagesForMailbox("account-two", "archive"),
    });

    invalidate.mockClear();
    act(() => handlers.get("message-content-changed")?.({
      payload: { accountId: "account-two", messageId: "message-one" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: messageQueryKeys.account("account-two") });

    invalidate.mockClear();
    act(() => handlers.get("pending-operation-changed")?.({
      payload: { accountId: "account-two" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.mailboxes("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.messagesForAccount("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: messageQueryKeys.account("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.pendingOperations("account-two") });

    invalidate.mockClear();
    act(() => handlers.get("contacts-changed")?.({
      payload: { accountId: "account-two" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.contactsForAccount("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.messagesForAccount("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: messageQueryKeys.account("account-two") });

    setQueryData.mockClear();
    invalidate.mockClear();
    act(() => {
      for (let completed = 1; completed <= 1_000; completed += 1) {
        handlers.get("sync-progress")?.({
          payload: {
            accountId: "account-two",
            phase: "summaries",
            completed,
            total: 1_000,
            currentMailboxName: "Inbox",
            errorCode: null,
            revision: completed,
          } as never,
        });
      }
    });
    expect(setQueryData).toHaveBeenCalledTimes(1_000);
    expect(client.getQueryData(mailQueryKeys.syncProgress("account-two")))
      .toEqual(expect.objectContaining({ completed: 1_000, revision: 1_000 }));
    act(() => handlers.get("sync-progress")?.({
      payload: {
        accountId: "account-two",
        phase: "summaries",
        completed: 999,
        total: 1_000,
        currentMailboxName: "Inbox",
        errorCode: null,
        revision: 999,
      } as never,
    }));
    expect(client.getQueryData(mailQueryKeys.syncProgress("account-two")))
      .toEqual(expect.objectContaining({ completed: 1_000, revision: 1_000 }));
    expect(invalidate).not.toHaveBeenCalledWith({
      queryKey: mailQueryKeys.syncProgress("account-two"),
    });

    invalidate.mockClear();
    act(() => handlers.get("account-runtime-status-changed")?.({
      payload: { accountId: "account-two" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.accountRuntimes });

    rerender({ selectedAccountId: "account-two", selectedMailboxId: "archive" });
    expect(listenMock).toHaveBeenCalledTimes(9);
    invalidate.mockClear();
    act(() => handlers.get("send-job-changed")?.({
      payload: {
        accountId: "account-two",
        jobId: "job-one",
        status: "sent",
        subject: "Hello",
      } as never,
    }));
    expect(onSent).toHaveBeenCalledWith({ id: "job-one", subject: "Hello" });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.drafts("account-two") });

    act(() => handlers.get("open-mail-location")?.({
      payload: {
        accountId: "account-two",
        mailboxId: "archive",
        messageId: "message-one",
      } as never,
    }));
    expect(onNavigate).toHaveBeenCalledWith({
      accountId: "account-two",
      mailboxId: "archive",
      messageId: "message-one",
    });

    unmount();
    await waitFor(() => disposers.forEach((dispose) => expect(dispose).toHaveBeenCalledOnce()));
  });
});
