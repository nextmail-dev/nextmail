import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <RecipientField
          label="To"
          addresses={[address]}
          input=""
          onInputChange={vi.fn()}
          onCommit={vi.fn()}
          onRemove={onRemove}
          onEditLast={onEditLast}
        />
      </QueryClientProvider>,
    );

    fireEvent.keyDown(screen.getByRole("combobox", { name: "To" }), { key: "Backspace" });

    expect(onEditLast).toHaveBeenCalledWith(address, 0);
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("commits immediately when a delimiter is pressed", () => {
    const onCommit = vi.fn();
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <RecipientField
          label="To"
          addresses={[]}
          input="alice@example.com"
          onInputChange={vi.fn()}
          onCommit={onCommit}
          onRemove={vi.fn()}
          onEditLast={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.keyDown(screen.getByRole("combobox", { name: "To" }), { key: "," });
    expect(onCommit).toHaveBeenCalledOnce();
  });

  it("offers account-local contacts and selects one without committing free text", async () => {
    vi.mocked(api.listContactSuggestions).mockResolvedValue({ contacts: [{
      id: "contact-one",
      name: "Alice Local",
      email: "alice@example.com",
      revision: 1,
      createdAt: 1,
      updatedAt: 1,
    }], groups: [] });
    const onSelectContacts = vi.fn();
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
          onSelectContacts={onSelectContacts}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole("option", { name: /Alice Local/ }));
    expect(api.listContactSuggestions).toHaveBeenCalledWith("account-one", "ali", 8);
    expect(onSelectContacts).toHaveBeenCalledWith([expect.objectContaining({ id: "contact-one" })]);
  });

  it("selects contact suggestions with arrow keys and Enter", async () => {
    vi.mocked(api.listContactSuggestions).mockResolvedValue({ contacts: [
      {
        id: "contact-one",
        name: "Alice Local",
        email: "alice@example.com",
        revision: 1,
        createdAt: 1,
        updatedAt: 1,
      },
      {
        id: "contact-two",
        name: "Bob Local",
        email: "bob@example.com",
        revision: 1,
        createdAt: 1,
        updatedAt: 1,
      },
    ], groups: [] });
    const onCommit = vi.fn();
    const onSelectContacts = vi.fn();
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <RecipientField
          accountId="account-one"
          label="To"
          addresses={[]}
          input="local"
          onInputChange={vi.fn()}
          onCommit={onCommit}
          onRemove={vi.fn()}
          onEditLast={vi.fn()}
          onSelectContacts={onSelectContacts}
        />
      </QueryClientProvider>,
    );
    const input = screen.getByRole("combobox", { name: "To" });
    const options = await screen.findAllByRole("option");

    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(options[1]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onSelectContacts).toHaveBeenCalledWith([expect.objectContaining({ id: "contact-one" })]);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("expands a group once, excludes existing addresses, and drops suggestions on account changes", async () => {
    const alice = { id: "a", name: "Alice", email: "alice@example.com", revision: 1, createdAt: 1, updatedAt: 1 };
    const bob = { ...alice, id: "b", name: "Bob", email: "bob@example.com" };
    vi.mocked(api.listContactSuggestions).mockImplementation(async (accountId) => ({
      contacts: [], groups: accountId === "account-one" ? [{ group: { id: "team", name: "Project Team", revision: 1, memberCount: 2 }, members: [alice, bob] }] : [],
    }));
    const onCommit = vi.fn();
    const onSelectContacts = vi.fn();
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const field = (accountId: string) => (
      <QueryClientProvider client={queryClient}>
        <RecipientField accountId={accountId} label="To" addresses={[{ name: "Existing", email: "ALICE@example.com" }]} input="Project"
          onInputChange={vi.fn()} onCommit={onCommit} onRemove={vi.fn()} onEditLast={vi.fn()} onSelectContacts={onSelectContacts} />
      </QueryClientProvider>
    );
    const { rerender } = render(field("account-one"));
    await screen.findByRole("option", { name: /Project Team/ });
    const input = screen.getByRole("combobox", { name: "To" });
    fireEvent.keyDown(input, { key: " " });
    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSelectContacts).toHaveBeenCalledExactlyOnceWith([bob]);
    rerender(field("account-two"));
    await waitFor(() => expect(api.listContactSuggestions).toHaveBeenCalledWith("account-two", "Project", 8));
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
  });
});
