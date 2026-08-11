import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { RawMessageApp } from "./RawMessageApp";

let locationListener: ((event: { payload: { accountId: string; messageId: string } }) => void) | null = null;

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name, listener) => {
    locationListener = listener;
    return Promise.resolve(vi.fn());
  }),
}));

vi.mock("@/app/api", () => ({
  api: {
    requestRawMessage: vi.fn((accountId: string, messageId: string) =>
      Promise.resolve(`source:${accountId}:${messageId}`)),
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
  locationListener = null;
  vi.clearAllMocks();
});

describe("RawMessageApp", () => {
  it("reloads raw content when the singleton window receives a new target", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <RawMessageApp accountId="account-one" messageId="message-one" />
      </QueryClientProvider>,
    );

    const source = await screen.findByText("source:account-one:message-one");
    expect(source.closest(".rounded-lg")).toHaveClass(
      "border",
      "bg-muted/50",
      "shadow-[var(--shadow-control)]",
    );
    act(() => {
      locationListener?.({
        payload: { accountId: "account-two", messageId: "message-two" },
      });
    });
    expect(await screen.findByText("source:account-two:message-two")).toBeInTheDocument();
    expect(api.requestRawMessage).toHaveBeenLastCalledWith("account-two", "message-two");
  });
});
