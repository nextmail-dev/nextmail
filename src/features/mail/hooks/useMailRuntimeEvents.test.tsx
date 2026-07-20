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
    const onSent = vi.fn();
    const { rerender, unmount } = renderHook(
      ({ selectedAccountId }) => useMailRuntimeEvents({ selectedAccountId, onSent }),
      {
        initialProps: { selectedAccountId: "account-one" },
        wrapper: createWrapper(client),
      },
    );
    await waitFor(() => expect(handlers.size).toBe(5));

    act(() => handlers.get("mailbox-changed")?.({
      payload: { accountId: "account-two", mailboxId: "archive" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.mailboxes("account-two") });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.messagesForMailbox("account-two", "archive") });

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
    act(() => handlers.get("sync-progress")?.({
      payload: { accountId: "account-two" } as never,
    }));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: mailQueryKeys.syncProgress("account-two") });

    rerender({ selectedAccountId: "account-two" });
    expect(listenMock).toHaveBeenCalledTimes(5);
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

    unmount();
    await waitFor(() => disposers.forEach((dispose) => expect(dispose).toHaveBeenCalledOnce()));
  });
});
