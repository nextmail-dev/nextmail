import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import { RecipientField } from "./RecipientField";

vi.mock("@/app/api", () => ({ api: { listContactSuggestions: vi.fn() } }));

afterEach(cleanup);

describe("RecipientField", () => {
  it("restores the last tag to the input for editing on Backspace", () => {
    const address = { name: "Alice", email: "alice@example.com" };
    const onEditLast = vi.fn();
    const onRemove = vi.fn();
    render(
      <RecipientField
        label="To"
        addresses={[address]}
        input=""
        onInputChange={vi.fn()}
        onCommit={vi.fn()}
        onRemove={onRemove}
        onEditLast={onEditLast}
      />,
    );

    fireEvent.keyDown(screen.getByRole("textbox", { name: "To" }), { key: "Backspace" });

    expect(onEditLast).toHaveBeenCalledWith(address, 0);
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("commits immediately when a delimiter is pressed", () => {
    const onCommit = vi.fn();
    render(
      <RecipientField
        label="To"
        addresses={[]}
        input="alice@example.com"
        onInputChange={vi.fn()}
        onCommit={onCommit}
        onRemove={vi.fn()}
        onEditLast={vi.fn()}
      />,
    );

    fireEvent.keyDown(screen.getByRole("textbox", { name: "To" }), { key: "," });
    expect(onCommit).toHaveBeenCalledOnce();
  });

  it("offers account-local contacts and selects one without committing free text", async () => {
    vi.mocked(api.listContactSuggestions).mockResolvedValue([{
      id: "contact-one",
      name: "Alice Local",
      email: "alice@example.com",
      revision: 1,
      createdAt: 1,
      updatedAt: 1,
    }]);
    const onSelectContact = vi.fn();
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <RecipientField
          accountId="account-one"
          label="To"
          addresses={[]}
          input="ali"
          onInputChange={vi.fn()}
          onCommit={vi.fn()}
          onRemove={vi.fn()}
          onEditLast={vi.fn()}
          onSelectContact={onSelectContact}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("option", { name: /Alice Local/ }));
    expect(api.listContactSuggestions).toHaveBeenCalledWith("account-one", "ali", 8);
    expect(onSelectContact).toHaveBeenCalledWith(expect.objectContaining({ id: "contact-one" }));
  });
});
