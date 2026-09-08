import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { ContactGroupManager } from "./ContactGroupManager";

vi.mock("@/app/api", () => ({
  api: { listContactGroups: vi.fn(), getContactGroup: vi.fn(), saveContactGroup: vi.fn(), deleteContactGroup: vi.fn(), listContacts: vi.fn() },
  normalizeCommandError: (error: unknown) => error,
}));
const alice = { id: "alice", name: "Alice", email: "alice@example.com", revision: 1, createdAt: 1, updatedAt: 1 };
const bob = { ...alice, id: "bob", name: "Bob", email: "bob@example.com" };
const carol = { ...alice, id: "carol", name: "Carol", email: "carol@example.com" };
const detail = { group: { id: "team", name: "Team", revision: 3, memberCount: 1 }, members: [carol] };

beforeAll(async () => { await i18n.changeLanguage("en-US"); });
beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(api.listContactGroups).mockResolvedValue([detail.group]);
  vi.mocked(api.getContactGroup).mockResolvedValue(detail);
  vi.mocked(api.saveContactGroup).mockResolvedValue(detail);
  vi.mocked(api.listContacts).mockImplementation(async (_accountId, query, cursor) => ({
    items: query ? [bob] : cursor ? [bob] : [alice], nextCursor: query || cursor ? null : "next", total: 2,
  }));
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

function renderManager(onClose = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><ContactGroupManager accountId="account-one" onClose={onClose} /></QueryClientProvider>);
}

describe("ContactGroupManager", () => {
  it("retains member choices through search and pagination and saves one account-scoped draft", async () => {
    renderManager();
    fireEvent.click(screen.getByRole("button", { name: "New group" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Group name" }), { target: { value: "Project Team" } });
    fireEvent.click(await screen.findByRole("checkbox", { name: "Alice" }));
    fireEvent.click(screen.getByRole("button", { name: "Load more contacts" }));
    fireEvent.click(await screen.findByRole("checkbox", { name: "Bob" }));
    fireEvent.change(screen.getByRole("searchbox", { name: "Search contacts" }), { target: { value: "Bob" } });
    await waitFor(() => expect(api.listContacts).toHaveBeenCalledWith("account-one", "Bob", null, 50));
    fireEvent.change(screen.getByRole("searchbox", { name: "Search contacts" }), { target: { value: "" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Selected only" }));
    expect(screen.getByRole("checkbox", { name: "Alice" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Bob" })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.saveContactGroup).toHaveBeenCalledWith("account-one", null, { name: "Project Team", contactIds: ["alice", "bob"] }, null));
  });

  it("preserves unloaded members and the edit revision, reports conflicts, and guards unsaved navigation", async () => {
    const onClose = vi.fn();
    vi.mocked(api.saveContactGroup).mockRejectedValue({ code: "contact_group.conflict" });
    renderManager(onClose);
    fireEvent.click(await screen.findByRole("button", { name: /Team/ }));
    const name = await screen.findByRole("textbox", { name: "Group name" });
    fireEvent.change(name, { target: { value: "Renamed" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Selected only" }));
    expect(screen.getByRole("checkbox", { name: "Carol" })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.saveContactGroup).toHaveBeenCalledWith("account-one", "team", { name: "Renamed", contactIds: ["carol"] }, 3));
    expect(await screen.findByRole("alert")).toHaveTextContent("changed elsewhere");
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(await screen.findByRole("dialog", { name: "Discard unsaved changes?" })).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
    expect(name).toHaveValue("Renamed");
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("confirms group deletion without invoking contact deletion", async () => {
    renderManager();
    fireEvent.click(await screen.findByRole("button", { name: /Team/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete group" }));
    expect(screen.getByText(/Its contacts will be kept/)).toBeInTheDocument();
    expect(api.deleteContactGroup).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(api.deleteContactGroup).toHaveBeenCalledWith("account-one", "team", 3));
  });
});
