import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import { SettingsApp } from "./SettingsApp";

vi.mock("@/app/api", () => ({
  api: {
    getPreferences: vi.fn().mockResolvedValue({
      theme: "system",
      accentColor: "#2563eb",
      language: "en-US",
    }),
    listAccountSummaries: vi.fn().mockResolvedValue([
      { id: "account-one", email: "alice@example.com", displayName: "Alice" },
    ]),
    getAppAbout: vi.fn().mockResolvedValue({ name: "NextMail", version: "0.1.0" }),
    getReadingPreferences: vi.fn().mockResolvedValue({ autoLoadRemoteImages: false }),
    setAppearancePreferences: vi.fn(),
    setReadingPreferences: vi.fn().mockImplementation((preferences) => Promise.resolve(preferences)),
  },
  normalizeCommandError: vi.fn(() => ({ code: "common.unexpected_error", params: {}, retryable: false })),
}));

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("SettingsApp", () => {
  it("renders the independent settings shell after loading preferences and accounts", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "General" })).toBeInTheDocument();
    expect(screen.getByText("Language")).toBeInTheDocument();
  });

  it("exposes the persisted remote-image preference in Reading", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <SettingsApp />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Reading" }));
    expect(screen.getByText("Automatically load remote images")).toBeInTheDocument();
    expect(screen.getByRole("checkbox")).not.toBeChecked();
  });
});
