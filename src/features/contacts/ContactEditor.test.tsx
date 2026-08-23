import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { DirectContactEditor } from "./ContactEditor";

vi.mock("@/app/api", () => ({
  api: {
    getContactSummary: vi.fn(),
    updateContactName: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));

const contact = {
  id: "contact-one",
  name: "Alice",
  email: "alice@example.com",
  revision: 4,
  createdAt: 1,
  updatedAt: 2,
};

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getContactSummary).mockResolvedValue(contact);
  vi.mocked(api.updateContactName).mockResolvedValue({ ...contact, name: "Alice Updated", revision: 5 });
});

afterEach(cleanup);

describe("DirectContactEditor", () => {
  it("loads only the contact summary and updates it without opening the contacts workspace", async () => {
    const onClose = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <DirectContactEditor accountId="account-one" contactId="contact-one" onClose={onClose} />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(api.getContactSummary).toHaveBeenCalledWith("account-one", "contact-one"));
    const name = await screen.findByRole("textbox", { name: "Name" });
    expect(screen.getByRole("textbox", { name: "Email address" })).toBeDisabled();
    fireEvent.change(name, { target: { value: "Alice Updated" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(api.updateContactName).toHaveBeenCalledWith(
      "account-one", "contact-one", "Alice Updated", 4,
    ));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});
