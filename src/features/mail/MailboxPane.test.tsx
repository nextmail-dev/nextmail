import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "../../app/i18n";
import type { MailboxSummary } from "../../app/types";
import { flattenMailboxHierarchy, MailboxPane } from "./MailboxPane";

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("MailboxPane", () => {
  it("keeps collapsed compose and mailbox icons in fixed square controls", () => {
    const inbox: MailboxSummary = {
      id: "inbox",
      accountId: "account-one",
      name: "INBOX",
      delimiter: null,
      role: "inbox",
      selectable: true,
      totalCount: 3,
      unreadCount: 1,
      revision: 1,
    };
    render(
      <MailboxPane
        mailboxes={[inbox]}
        selectedMailboxId="inbox"
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
        collapsed
      />,
    );

    const compose = screen.getByRole("button", { name: "New message" });
    const mailbox = screen.getByRole("button", { name: "Inbox" });
    expect(compose).toHaveClass("mx-auto", "size-10", "p-0");
    expect(mailbox).toHaveClass("mx-auto", "size-10", "p-0");
    expect(mailbox.querySelector("svg")).toHaveClass("size-[18px]", "shrink-0");
  });

  it("auto-hides the folder scrollbar without reserving viewport width", () => {
    const inbox: MailboxSummary = {
      id: "inbox",
      accountId: "account-one",
      name: "INBOX",
      delimiter: null,
      role: "inbox",
      selectable: true,
      totalCount: 3,
      unreadCount: 1,
      revision: 1,
    };
    const { container } = render(
      <MailboxPane
        mailboxes={[inbox]}
        selectedMailboxId="inbox"
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    const scrollArea = container.querySelector('[data-scrollbar-auto-hide="true"]');
    expect(scrollArea).toBeInTheDocument();
    const viewport = scrollArea?.querySelector(".native-scrollbar-hidden");
    expect(scrollArea).toHaveClass("-mr-3");
    expect(viewport).not.toHaveClass("pr-3", "pr-1.5");
    expect(viewport?.firstElementChild).toHaveClass("pr-3");
    expect(within(container).getByRole("button", { name: "Inbox" }).parentElement).toHaveClass(
      "h-9",
      "shadow-[inset_2px_0_0_var(--primary)]",
    );
  });

  it("builds nested folders from the server delimiter instead of guessing separators", () => {
    const mailboxes: MailboxSummary[] = [
      { id: "child", accountId: "account-one", name: "Other/Archive", delimiter: "/", role: "other", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
      { id: "root", accountId: "account-one", name: "Other", delimiter: "/", role: "other", selectable: false, totalCount: 0, unreadCount: 0, revision: 1 },
      { id: "literal", accountId: "account-one", name: "News/2026", delimiter: ".", role: "other", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
    ];

    const items = flattenMailboxHierarchy(mailboxes);
    expect(items.map(({ mailbox, depth, displayName }) => [mailbox.id, depth, displayName])).toEqual([
      ["root", 0, "Other"],
      ["child", 1, "Archive"],
      ["literal", 0, "News/2026"],
    ]);
  });

  it("keeps parent folders selectable and lets their children collapse", () => {
    const onSelect = vi.fn();
    const mailboxes: MailboxSummary[] = [
      { id: "root", accountId: "account-one", name: "Other", delimiter: "/", role: "other", selectable: false, totalCount: 2, unreadCount: 0, revision: 1 },
      { id: "child", accountId: "account-one", name: "Other/Archive", delimiter: "/", role: "other", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
    ];
    render(
      <MailboxPane
        mailboxes={mailboxes}
        selectedMailboxId=""
        onSelect={onSelect}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    const parent = screen.getByRole("button", { name: /^Other$/ });
    expect(parent).toBeEnabled();
    fireEvent.click(parent);
    expect(onSelect).toHaveBeenCalledWith("root");

    const collapse = screen.getByRole("button", { name: "Collapse Other" });
    expect(collapse).toHaveClass("w-4");
    fireEvent.click(collapse);
    expect(screen.queryByRole("button", { name: "Archive" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Expand Other" }));
    expect(screen.getByRole("button", { name: "Archive" })).toBeInTheDocument();
    expect(onSelect).toHaveBeenCalledOnce();
  });

  it("does not capture an ordinary folder click before the long-press threshold", () => {
    const onSelect = vi.fn();
    const capturePointer = vi.fn();
    const originalCapture = Object.getOwnPropertyDescriptor(
      HTMLElement.prototype,
      "setPointerCapture",
    );
    Object.defineProperty(HTMLElement.prototype, "setPointerCapture", {
      configurable: true,
      value: capturePointer,
    });
    try {
      const { container } = render(
        <MailboxPane
          mailboxes={[{
            id: "archive",
            accountId: "account-one",
            name: "Archive",
            delimiter: "/",
            role: "other",
            selectable: true,
            totalCount: 1,
            unreadCount: 0,
            revision: 1,
          }]}
          selectedMailboxId=""
          onSelect={onSelect}
          onCompose={vi.fn()}
          onReceive={vi.fn()}
          receiving={false}
          onOpenSettings={vi.fn()}
        />,
      );

      const folder = within(container).getByRole("button", { name: "Archive" });
      fireEvent.pointerDown(folder, { button: 0, pointerId: 1, pointerType: "mouse" });
      expect(capturePointer).not.toHaveBeenCalled();
      fireEvent.pointerUp(folder, { button: 0, pointerId: 1, pointerType: "mouse" });
      fireEvent.click(folder);
      expect(onSelect).toHaveBeenCalledWith("archive");
    } finally {
      if (originalCapture) {
        Object.defineProperty(HTMLElement.prototype, "setPointerCapture", originalCapture);
      } else {
        delete (HTMLElement.prototype as Partial<HTMLElement>).setPointerCapture;
      }
    }
  });

  it("places receive beside the folder heading and settings at the pane bottom", () => {
    const onReceive = vi.fn();
    const onOpenSettings = vi.fn();
    const { container } = render(
      <MailboxPane
        mailboxes={[]}
        selectedMailboxId=""
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={onReceive}
        receiving={false}
        onOpenSettings={onOpenSettings}
      />,
    );

    const current = within(container);
    const folderHeading = current.getByText("Mail folders");
    const receive = current.getByRole("button", { name: "Receive" });
    const settings = current.getByRole("button", { name: "Settings" });
    expect(folderHeading.parentElement).toContainElement(receive);
    expect(folderHeading.parentElement).toHaveClass("pl-[26px]");
    expect(settings).toHaveClass("mt-auto");

    fireEvent.click(receive);
    fireEvent.click(settings);
    expect(onReceive).toHaveBeenCalledOnce();
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });

  it("disables manual receive while any account synchronization is active", () => {
    const { container } = render(
      <MailboxPane
        mailboxes={[]}
        selectedMailboxId=""
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving
        onOpenSettings={vi.fn()}
      />,
    );

    expect(within(container).getByRole("button", { name: "Receive" })).toBeDisabled();
  });

  it("offers folder management from the folder context menu", async () => {
    const onMarkFolderAllRead = vi.fn().mockResolvedValue(undefined);
    const archive: MailboxSummary = {
      id: "archive",
      accountId: "account-one",
      name: "Archive",
      delimiter: "/",
      role: "other",
      selectable: true,
      totalCount: 4,
      unreadCount: 2,
      revision: 1,
    };
    const { container } = render(
      <MailboxPane
        mailboxes={[archive]}
        selectedMailboxId="archive"
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onMarkFolderAllRead={onMarkFolderAllRead}
        onOpenSettings={vi.fn()}
      />,
    );

    fireEvent.contextMenu(within(container).getByRole("button", { name: "Archive" }));
    expect(await screen.findByRole("menuitem", { name: "New subfolder" })).toBeEnabled();
    expect(screen.getByRole("menuitem", { name: "Rename folder" })).toBeEnabled();
    expect(screen.getByRole("menuitem", { name: "Move folder" })).toBeEnabled();
    fireEvent.click(screen.getByRole("menuitem", { name: "Mark all as read" }));
    await waitFor(() => expect(onMarkFolderAllRead).toHaveBeenCalledWith("archive"));
  });

  it("keeps the unread view in the scrollable folder list with only mark-all-read in its menu", async () => {
    const onSelect = vi.fn();
    const onMarkAllUnreadRead = vi.fn().mockResolvedValue(undefined);
    const inbox: MailboxSummary = {
      id: "inbox",
      accountId: "account-one",
      name: "INBOX",
      delimiter: "/",
      role: "inbox",
      selectable: true,
      totalCount: 3,
      unreadCount: 2,
      revision: 1,
    };
    const { container } = render(
      <MailboxPane
        mailboxes={[inbox]}
        selectedMailboxId="__nextmail_unread__"
        onSelect={onSelect}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onMarkAllUnreadRead={onMarkAllUnreadRead}
        onOpenSettings={vi.fn()}
      />,
    );

    const unread = within(container).getByRole("button", { name: "Unread" });
    const scrollArea = container.querySelector('[data-scrollbar-auto-hide="true"]');
    expect(scrollArea).toContainElement(unread);
    expect(unread).toHaveTextContent("2");
    expect(unread.compareDocumentPosition(within(container).getByRole("button", { name: "Inbox" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    fireEvent.click(unread);
    expect(onSelect).toHaveBeenCalledWith("__nextmail_unread__");
    fireEvent.contextMenu(unread);
    const items = await screen.findAllByRole("menuitem");
    expect(items).toHaveLength(1);
    fireEvent.click(items[0]);
    await waitFor(() => expect(onMarkAllUnreadRead).toHaveBeenCalledOnce());
  });

  it("renders fixed virtual favorites and persists real folder favorites from context menus", async () => {
    const onSetFavorite = vi.fn().mockResolvedValue(undefined);
    const mailboxes: MailboxSummary[] = [
      { id: "inbox", accountId: "account-one", name: "INBOX", delimiter: "/", role: "inbox", selectable: true, totalCount: 2, unreadCount: 1, isFavorite: true, revision: 1 },
      { id: "archive", accountId: "account-one", name: "Archive", delimiter: "/", role: "archive", selectable: true, totalCount: 1, unreadCount: 0, isFavorite: false, revision: 1 },
    ];
    const { container } = render(
      <MailboxPane
        mailboxes={mailboxes}
        selectedMailboxId=""
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onSetFavorite={onSetFavorite}
        onOpenSettings={vi.fn()}
      />,
    );

    const favoritesLabel = within(container).getByText("Favorites");
    const foldersLabel = within(container).getByText("Mail folders");
    expect(favoritesLabel.compareDocumentPosition(foldersLabel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const starred = within(container).getByRole("button", { name: "Starred" });
    expect(starred).toBeInTheDocument();
    expect(within(container).getByRole("button", { name: "Unread" })).toBeInTheDocument();
    const inboxRows = within(container).getAllByRole("button", { name: "Inbox" });
    expect(inboxRows).toHaveLength(2);
    expect(inboxRows[0].compareDocumentPosition(starred) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    fireEvent.contextMenu(inboxRows[0]);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from favorites" }));
    await waitFor(() => expect(onSetFavorite).toHaveBeenCalledWith("inbox", false));

    fireEvent.contextMenu(within(container).getByRole("button", { name: "Archive" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Add to favorites" }));
    await waitFor(() => expect(onSetFavorite).toHaveBeenCalledWith("archive", true));
  });

  it("highlights only the clicked occurrence of a favorite mailbox", () => {
    const { container } = render(
      <MailboxPane
        mailboxes={[{
          id: "inbox",
          accountId: "account-one",
          name: "INBOX",
          delimiter: "/",
          role: "inbox",
          selectable: true,
          totalCount: 2,
          unreadCount: 1,
          isFavorite: true,
          revision: 1,
        }]}
        selectedMailboxId="inbox"
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    const inboxRows = within(container).getAllByRole("button", { name: "Inbox" });
    const favoriteRow = inboxRows[0];
    const folderRow = inboxRows[1].parentElement;
    expect(favoriteRow).not.toHaveClass("bg-primary/10");
    expect(folderRow).toHaveClass("bg-primary/10");

    fireEvent.click(favoriteRow);
    expect(favoriteRow).toHaveClass("bg-primary/10");
    expect(folderRow).not.toHaveClass("bg-primary/10");

    fireEvent.click(inboxRows[1]);
    expect(favoriteRow).not.toHaveClass("bg-primary/10");
    expect(folderRow).toHaveClass("bg-primary/10");
  });

  it("selects a leaf folder from the reserved expander area", () => {
    const onSelect = vi.fn();
    const { container } = render(
      <MailboxPane
        mailboxes={[{
          id: "inbox",
          accountId: "account-one",
          name: "INBOX",
          delimiter: "/",
          role: "inbox",
          selectable: true,
          totalCount: 3,
          unreadCount: 0,
          revision: 1,
        }]}
        selectedMailboxId=""
        onSelect={onSelect}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    const inbox = within(container).getByRole("button", { name: "Inbox" });
    expect(inbox.previousElementSibling).toHaveClass("w-4");
    fireEvent.click(inbox.previousElementSibling!);
    expect(onSelect).toHaveBeenCalledWith("inbox");
  });

  it("moves folder selection with arrow keys instead of scrolling", () => {
    const onSelect = vi.fn();
    const { container } = render(
      <MailboxPane
        mailboxes={[
          { id: "inbox", accountId: "account-one", name: "INBOX", delimiter: "/", role: "inbox", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
          { id: "archive", accountId: "account-one", name: "Archive", delimiter: "/", role: "archive", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
        ]}
        selectedMailboxId="inbox"
        onSelect={onSelect}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    const current = within(container);
    const inbox = current.getByRole("button", { name: "Inbox" });
    const archive = current.getByRole("button", { name: "Archive" });
    fireEvent.keyDown(inbox, { key: "ArrowDown" });
    expect(onSelect).toHaveBeenCalledWith("archive");
    expect(archive).toHaveFocus();
  });

  it("shows a circle-and-line insertion marker while reordering folders", () => {
    vi.useFakeTimers();
    const originalElementFromPoint = document.elementFromPoint;
    try {
      const { container } = render(
        <MailboxPane
          mailboxes={[
            { id: "archive", accountId: "account-one", name: "Archive", delimiter: "/", role: "other", selectable: true, totalCount: 0, unreadCount: 0, revision: 1 },
            { id: "projects", accountId: "account-one", name: "Projects", delimiter: "/", role: "other", selectable: true, totalCount: 0, unreadCount: 0, revision: 1 },
          ]}
          selectedMailboxId=""
          onSelect={vi.fn()}
          onCompose={vi.fn()}
          onReceive={vi.fn()}
          receiving={false}
          onOpenSettings={vi.fn()}
        />,
      );
      const current = within(container);
      const source = current.getByRole("button", { name: "Archive" });
      const target = current.getByRole("button", { name: "Projects" }).closest<HTMLElement>("[data-mailbox-reorder-id]")!;
      target.getBoundingClientRect = () => ({
        x: 0, y: 100, top: 100, left: 0, right: 200, bottom: 136, width: 200, height: 36,
        toJSON: () => ({}),
      });
      Object.defineProperty(document, "elementFromPoint", {
        configurable: true,
        value: () => target,
      });

      fireEvent.pointerDown(source, { button: 0, pointerId: 1, pointerType: "mouse", clientX: 10, clientY: 10 });
      act(() => vi.advanceTimersByTime(360));
      fireEvent.pointerMove(source, { pointerId: 1, pointerType: "mouse", clientX: 10, clientY: 110 });

      const marker = target.querySelector('[data-mailbox-drop-indicator="before"]');
      expect(marker).toHaveClass("top-0", "-translate-y-1/2");
      expect(marker?.children[0]).toHaveClass("rounded-full", "border-2");
      expect(marker?.children[1]).toHaveClass("h-0.5", "bg-primary");
    } finally {
      Object.defineProperty(document, "elementFromPoint", {
        configurable: true,
        value: originalElementFromPoint,
      });
      vi.useRealTimers();
    }
  });

  it("restores interaction after closing a folder dialog opened from a context menu", async () => {
    const archive: MailboxSummary = {
      id: "archive",
      accountId: "account-one",
      name: "Archive",
      delimiter: "/",
      role: "other",
      selectable: true,
      totalCount: 1,
      unreadCount: 0,
      revision: 1,
    };
    const { container } = render(
      <MailboxPane
        mailboxes={[archive]}
        selectedMailboxId="archive"
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    fireEvent.contextMenu(within(container).getByRole("button", { name: "Archive" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename folder" }));
    expect(await screen.findByRole("dialog", { name: "Rename folder" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename folder" })).not.toBeInTheDocument());
    expect(document.querySelector(".app-dialog-overlay")).not.toBeInTheDocument();
    expect(document.body.style.pointerEvents).not.toBe("none");
  });

  it("renders and selects move destinations above the folder dialog", async () => {
    const mailboxes: MailboxSummary[] = [
      { id: "archive", accountId: "account-one", name: "Archive", delimiter: "/", role: "other", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
      { id: "projects", accountId: "account-one", name: "Projects", delimiter: "/", role: "other", selectable: true, totalCount: 1, unreadCount: 0, revision: 1 },
    ];
    const { container } = render(
      <MailboxPane
        mailboxes={mailboxes}
        selectedMailboxId="archive"
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    fireEvent.contextMenu(within(container).getByRole("button", { name: "Archive" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Move folder" }));
    expect(await screen.findByRole("dialog", { name: "Move folder" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("combobox", { name: "Destination" }));
    const option = await screen.findByRole("option", { name: "Projects" });
    expect(option.closest(".app-floating-content")).toBeInTheDocument();
    fireEvent.click(option);
    expect(screen.getByRole("combobox", { name: "Destination" })).toHaveTextContent("Projects");
  });

  it("shows message progress without exposing folder-count progress", () => {
    const baseProps = {
      mailboxes: [],
      selectedMailboxId: "",
      onSelect: vi.fn(),
      onCompose: vi.fn(),
      onReceive: vi.fn(),
      receiving: true,
      showProgress: true,
      onOpenSettings: vi.fn(),
    };
    const { rerender } = render(
      <MailboxPane
        {...baseProps}
        progress={{
          accountId: "account-one",
          phase: "folders",
          completed: 3,
          total: 9,
          currentMailboxName: "Archive",
          errorCode: null,
          revision: 1,
        }}
      />,
    );

    expect(screen.getByText("Synchronizing folder Archive")).toBeInTheDocument();
    expect(screen.queryByText(/3\/9/)).not.toBeInTheDocument();

    rerender(
      <MailboxPane
        {...baseProps}
        progress={{
          accountId: "account-one",
          phase: "summaries",
          completed: 3,
          total: 9,
          currentMailboxName: "Archive",
          errorCode: null,
          revision: 2,
        }}
      />,
    );
    expect(screen.getByText("Synchronizing Archive (3/9)")).toBeInTheDocument();
  });

  it("hides background sync progress unless it was manually requested", () => {
    const { container } = render(
      <MailboxPane
        mailboxes={[]}
        selectedMailboxId=""
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        onReceive={vi.fn()}
        receiving
        onOpenSettings={vi.fn()}
        progress={{
          accountId: "account-one",
          phase: "summaries",
          completed: 3,
          total: 9,
          currentMailboxName: "Archive",
          errorCode: null,
          revision: 1,
        }}
      />,
    );

    expect(within(container).queryByText("Synchronizing Archive (3/9)")).not.toBeInTheDocument();
  });
});
