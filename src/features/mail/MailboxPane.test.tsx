import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

    fireEvent.click(screen.getByRole("button", { name: "Collapse Other" }));
    expect(screen.queryByRole("button", { name: "Archive" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Expand Other" }));
    expect(screen.getByRole("button", { name: "Archive" })).toBeInTheDocument();
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

  it("shows message progress without exposing folder-count progress", () => {
    const baseProps = {
      mailboxes: [],
      selectedMailboxId: "",
      onSelect: vi.fn(),
      onCompose: vi.fn(),
      onReceive: vi.fn(),
      receiving: true,
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
});
