import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "./api";
import {
  appearanceQueryKey,
  useAppearanceEventBridge,
  useAppearancePreferences,
  useUpdateAppearancePreferences,
} from "./appearance";
import type { AppearancePreferences } from "./types";

const { listenMock } = vi.hoisted(() => ({ listenMock: vi.fn() }));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("./api", () => ({
  api: {
    getPreferences: vi.fn(),
    setAppearancePreferences: vi.fn(),
  },
}));

const initialPreferences: AppearancePreferences = {
  theme: "light",
  accentColor: "#2563eb",
  language: "en-US",
};

function createClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
      mutations: { retry: false },
    },
  });
}

function createWrapper(client: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getPreferences).mockResolvedValue(initialPreferences);
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("style");
});

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("style");
});

describe("appearance query state", () => {
  it("optimistically updates preferences and keeps the persisted response", async () => {
    const client = createClient();
    client.setQueryData(appearanceQueryKey, initialPreferences);
    const pending = deferred<AppearancePreferences>();
    vi.mocked(api.setAppearancePreferences).mockReturnValue(pending.promise);
    const optimistic: AppearancePreferences = {
      theme: "dark",
      accentColor: "#7c3aed",
      language: "zh-CN",
    };
    const persisted = { ...optimistic, accentColor: "#9333ea" };
    const { result } = renderHook(() => ({
      preferences: useAppearancePreferences(),
      mutation: useUpdateAppearancePreferences(),
    }), { wrapper: createWrapper(client) });

    act(() => result.current.mutation.mutate(optimistic));

    await waitFor(() => {
      expect(client.getQueryData(appearanceQueryKey)).toEqual(optimistic);
      expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    });

    pending.resolve(persisted);

    await waitFor(() => {
      expect(result.current.mutation.isSuccess).toBe(true);
      expect(client.getQueryData(appearanceQueryKey)).toEqual(persisted);
      expect(document.documentElement.style.getPropertyValue("--primary")).toBe("#9333ea");
    });
  });

  it("rolls the query cache and applied UI back when persistence fails", async () => {
    const client = createClient();
    client.setQueryData(appearanceQueryKey, initialPreferences);
    const pending = deferred<AppearancePreferences>();
    vi.mocked(api.setAppearancePreferences).mockReturnValue(pending.promise);
    const optimistic: AppearancePreferences = {
      theme: "dark",
      accentColor: "#dc2626",
      language: "zh-CN",
    };
    const { result } = renderHook(() => ({
      preferences: useAppearancePreferences(),
      mutation: useUpdateAppearancePreferences(),
    }), { wrapper: createWrapper(client) });

    act(() => result.current.mutation.mutate(optimistic));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));

    pending.reject(new Error("write failed"));

    await waitFor(() => {
      expect(result.current.mutation.isError).toBe(true);
      expect(client.getQueryData(appearanceQueryKey)).toEqual(initialPreferences);
      expect(document.documentElement).toHaveAttribute("data-theme", "light");
      expect(document.documentElement.style.getPropertyValue("--primary")).toBe("#2563eb");
    });
  });

  it("synchronizes an event into the local query client and disposes its listener", async () => {
    const client = createClient();
    const dispose = vi.fn();
    let receive: ((event: { payload: AppearancePreferences }) => void) | undefined;
    listenMock.mockImplementation((_event, handler) => {
      receive = handler;
      return Promise.resolve(dispose);
    });
    const eventPreferences: AppearancePreferences = {
      theme: "dark",
      accentColor: "#0f8a7b",
      language: "en-US",
    };
    const { unmount } = renderHook(() => useAppearanceEventBridge(), {
      wrapper: createWrapper(client),
    });

    act(() => receive?.({ payload: eventPreferences }));

    expect(client.getQueryData(appearanceQueryKey)).toEqual(eventPreferences);
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement.style.getPropertyValue("--primary")).toBe("#0f8a7b");

    unmount();
    await waitFor(() => expect(dispose).toHaveBeenCalledOnce());
  });
});
