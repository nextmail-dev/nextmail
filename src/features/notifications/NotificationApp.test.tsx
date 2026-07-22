import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import type { NewMailNotification } from "@/app/types";
import { NotificationApp, formatNotificationSender } from "./NotificationApp";

const { listenMock } = vi.hoisted(() => ({ listenMock: vi.fn() }));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@/app/appearance", () => ({ useAppearancePreferences: vi.fn() }));
vi.mock("@/app/api", () => ({
  api: {
    getNewMailNotification: vi.fn(),
    dismissNewMailNotification: vi.fn(),
    activateNewMailNotification: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({ code: "common.unexpected_error", params: {}, retryable: false })),
}));

const initialNotification: NewMailNotification = {
  id: "notification-one",
  accountId: "account-one",
  accountName: "Alice",
  accountEmail: "alice@example.com",
  mailboxId: "inbox",
  messageId: "message-one",
  senderName: "Sender",
  senderEmail: "sender@example.com",
  subject: "Initial subject",
};

let contentHandler: ((event: { payload: NewMailNotification }) => void) | null = null;

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  contentHandler = null;
  listenMock.mockImplementation((_eventName, handler) => {
    contentHandler = handler;
    return Promise.resolve(vi.fn());
  });
  vi.mocked(api.getNewMailNotification).mockResolvedValue(initialNotification);
  vi.mocked(api.dismissNewMailNotification).mockResolvedValue(undefined);
  vi.mocked(api.activateNewMailNotification).mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function renderNotification() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <NotificationApp notificationId="notification-one" />
    </QueryClientProvider>,
  );
}

describe("NotificationApp", () => {
  it("renders minimal mail details and replaces content in the same window", async () => {
    renderNotification();
    expect(await screen.findByText("Initial subject")).toBeInTheDocument();
    expect(screen.getByText("New mail · Alice")).toBeInTheDocument();
    expect(screen.getByText("Sender <sender@example.com>")).toBeInTheDocument();

    act(() => contentHandler?.({
      payload: { ...initialNotification, subject: "Replacement subject" },
    }));
    expect(await screen.findByText("Replacement subject")).toBeInTheDocument();
    expect(screen.queryByText("Initial subject")).not.toBeInTheDocument();
  });

  it("activates or dismisses through narrow notification commands", async () => {
    const { unmount } = renderNotification();
    fireEvent.click(await screen.findByRole("button", { name: "Open message: Initial subject" }));
    await waitFor(() => expect(api.activateNewMailNotification).toHaveBeenCalledWith("notification-one"));

    unmount();
    renderNotification();
    fireEvent.click(await screen.findByRole("button", { name: "Dismiss notification" }));
    await waitFor(() => expect(api.dismissNewMailNotification).toHaveBeenCalledWith("notification-one"));
  });

  it("formats missing sender names without empty angle brackets", () => {
    expect(formatNotificationSender({ senderName: null, senderEmail: "sender@example.com" }))
      .toBe("sender@example.com");
    expect(formatNotificationSender({ senderName: "Sender", senderEmail: "" })).toBe("Sender");
  });
});
