import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { UpdateWindowApp } from "./UpdateWindowApp";

vi.mock("@/app/api", () => ({
  api: {
    getAvailableUpdate: vi.fn().mockResolvedValue({
      available: true,
      currentVersion: "0.2.3",
      version: "0.2.4",
      notes: "## Fixes\n\n- Safer updates",
    }),
    installUpdate: vi.fn().mockResolvedValue(undefined),
  },
  normalizeCommandError: vi.fn(() => ({ code: "common.unexpected_error", params: {}, retryable: false })),
}));

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("UpdateWindowApp", () => {
  it("renders the pending update and starts the signed installer", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <UpdateWindowApp />
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "NextMail 0.2.4 is available" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Fixes" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Download and install" }));
    await waitFor(() => expect(api.installUpdate).toHaveBeenCalledOnce());
  });
});
