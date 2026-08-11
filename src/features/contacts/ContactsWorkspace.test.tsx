import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import { ContactsWorkspace } from "./ContactsWorkspace";

vi.mock("@/app/api", () => ({
  api: {
    listContacts: vi.fn(),
    getContactDetail: vi.fn(),
    createContact: vi.fn(),
    updateContactName: vi.fn(),
    deleteContacts: vi.fn(),
    openContactComposer: vi.fn(),
    getReadingPreferences: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));

const contact = {
  id: "contact-one",
  name: "Alice Local",
  email: "alice@example.com",
  revision: 2,
  createdAt: 1,
  updatedAt: 2,
};

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.listContacts).mockResolvedValue({ items: [contact], nextCursor: null, total: 1 });
  vi.mocked(api.getContactDetail).mockResolvedValue({
    contact,
    recentMessages: [{
      messageId: "message-one",
      mailboxId: "inbox",
      subject: "Project update",
      receivedAt: 3,
    }],
  });
  vi.mocked(api.openContactComposer).mockResolvedValue("draft-one");
  vi.mocked(api.deleteContacts).mockResolvedValue(undefined);
  vi.mocked(api.getReadingPreferences).mockResolvedValue({
    autoLoadRemoteImages: false,
    autoOpenDownloadedAttachments: true,
    autoLoadMoreMessages: true,
    autoLoadMoreContacts: true,
  });
});

afterEach(cleanup);

describe("ContactsWorkspace", () => {
  it("loads an account contact, opens composer, and navigates to recent mail", async () => {
    const onNavigateToMessage = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <ContactsWorkspace
          accountId="account-one"
          listPaneWidth={360}
          listPaneMax={520}
          onListPaneWidthChange={vi.fn()}
          onNavigateToMessage={onNavigateToMessage}
        />
      </QueryClientProvider>,
    );

    const contactRow = await screen.findByRole("button", { name: /Alice Local/ });
    fireEvent.contextMenu(contactRow);
    fireEvent.click(await screen.findByText("Edit contact"));
    expect(await screen.findByRole("heading", { name: "Edit contact" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(contactRow);
    expect(contactRow).toHaveClass("bg-selection", "before:w-[3px]", "cursor-default");
    expect(contactRow).toHaveClass("py-3", "focus-visible:ring-1");
    expect(contactRow.querySelector("span.grid")).toHaveClass("size-9");
    expect(await screen.findByRole("heading", { name: "Alice Local", level: 1 })).toHaveClass(
      "break-words",
      "text-2xl",
      "lg:text-2xl",
    );
    fireEvent.click(screen.getByRole("button", { name: "Send email" }));
    await waitFor(() => expect(api.openContactComposer).toHaveBeenCalledWith("account-one", "contact-one"));
    fireEvent.click(screen.getByRole("button", { name: /Project update/ }));
    expect(onNavigateToMessage).toHaveBeenCalledWith({
      accountId: "account-one",
      mailboxId: "inbox",
      messageId: "message-one",
    });
  });

  it("deletes the current multi-selection from the row context menu", async () => {
    const secondContact = { ...contact, id: "contact-two", name: "Bob Local", email: "bob@example.com" };
    vi.mocked(api.listContacts).mockResolvedValue({ items: [contact, secondContact], nextCursor: null, total: 2 });
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <ContactsWorkspace
          accountId="account-one"
          listPaneWidth={360}
          listPaneMax={520}
          onListPaneWidthChange={vi.fn()}
          onNavigateToMessage={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const alice = await screen.findByRole("button", { name: /Alice Local/ });
    const bob = await screen.findByRole("button", { name: /Bob Local/ });
    const scrollArea = alice.closest('[data-scrollbar-auto-hide="true"]');
    expect(scrollArea).toBeInTheDocument();
    expect(scrollArea?.querySelector(".native-scrollbar-hidden")).not.toHaveClass("pr-2");
    expect(alice.parentElement).toHaveClass("after:inset-x-5", "after:h-px", "after:bg-border/80");
    expect(alice.parentElement).not.toHaveClass("after:hidden");
    expect(bob.parentElement).toHaveClass("after:hidden");
    fireEvent.click(alice);
    fireEvent.click(bob, { ctrlKey: true });
    expect(alice).toHaveAttribute("aria-pressed", "true");
    expect(bob).toHaveAttribute("aria-pressed", "true");
    fireEvent.contextMenu(bob);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete 2 contacts" }));

    await waitFor(() => expect(api.deleteContacts).toHaveBeenCalledWith(
      "account-one",
      ["contact-one", "contact-two"],
    ));
  });

  it("keeps the explicit load-more action when contact auto-pagination is disabled", async () => {
    vi.mocked(api.getReadingPreferences).mockResolvedValue({
      autoLoadRemoteImages: false,
      autoOpenDownloadedAttachments: true,
      autoLoadMoreMessages: true,
      autoLoadMoreContacts: false,
    });
    vi.mocked(api.listContacts).mockResolvedValue({ items: [contact], nextCursor: "next-page", total: 2 });
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <ContactsWorkspace
          accountId="account-one"
          listPaneWidth={360}
          listPaneMax={520}
          onListPaneWidthChange={vi.fn()}
          onNavigateToMessage={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("button", { name: "Load more contacts" })).toBeInTheDocument();
  });

  it("loads the next contact page near the end of the list when enabled", async () => {
    const secondContact = { ...contact, id: "contact-two", name: "Bob Local", email: "bob@example.com" };
    vi.mocked(api.listContacts).mockImplementation(async (_accountId, _search, cursor) => cursor
      ? { items: [secondContact], nextCursor: null, total: 2 }
      : { items: [contact], nextCursor: "next-page", total: 2 });
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={queryClient}>
        <ContactsWorkspace
          accountId="account-one"
          listPaneWidth={360}
          listPaneMax={520}
          onListPaneWidthChange={vi.fn()}
          onNavigateToMessage={vi.fn()}
        />
      </QueryClientProvider>,
    );

    await screen.findByRole("button", { name: /Alice Local/ });
    const viewport = container.querySelector<HTMLElement>(".native-scrollbar-hidden");
    expect(viewport).not.toBeNull();
    Object.defineProperties(viewport!, {
      scrollHeight: { configurable: true, value: 1_000 },
      clientHeight: { configurable: true, value: 500 },
      scrollTop: { configurable: true, value: 360, writable: true },
    });
    fireEvent.scroll(viewport!);

    expect(await screen.findByRole("button", { name: /Bob Local/ })).toBeInTheDocument();
    expect(api.listContacts).toHaveBeenLastCalledWith("account-one", "", "next-page", 50);
  });
});
