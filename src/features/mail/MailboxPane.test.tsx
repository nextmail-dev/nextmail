import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "../../app/i18n";
import type { DraftListItem, MailboxSummary } from "../../app/types";
import { flattenMailboxHierarchy, MailboxPane } from "./MailboxPane";

const draft: DraftListItem = {
  id: "draft-one",
  accountId: "account-one",
  subject: "Status update",
  recipients: [{ name: "Alice", email: "alice@example.com" }],
  updatedAt: 1,
};

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("MailboxPane draft actions", () => {
  it("keeps the delete action separate from opening the draft", async () => {
    const onOpenDraft = vi.fn();
    const onDeleteDraft = vi.fn().mockResolvedValue(undefined);
    render(
      <MailboxPane
        mailboxes={[]}
        selectedMailboxId=""
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        drafts={[draft]}
        onOpenDraft={onOpenDraft}
        onDeleteDraft={onDeleteDraft}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
      />,
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: "Open a local draft" }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete draft" }));

    expect(onOpenDraft).not.toHaveBeenCalled();
    expect(onDeleteDraft).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("menuitem", { name: "Click again to delete this draft" }));

    await waitFor(() => expect(onDeleteDraft).toHaveBeenCalledWith("draft-one"));
    expect(onOpenDraft).not.toHaveBeenCalled();
  });

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
        drafts={[]}
        onOpenDraft={vi.fn()}
        onDeleteDraft={vi.fn()}
        onReceive={vi.fn()}
        receiving={false}
        onOpenSettings={vi.fn()}
        collapsed
      />,
    );

    const compose = screen.getByRole("button", { name: "New message" });
    const mailbox = screen.getByRole("button", { name: "Inbox" });
    expect(compose).toHaveClass("mx-auto", "size-11", "p-0");
    expect(mailbox).toHaveClass("mx-auto", "size-11", "p-0");
    expect(mailbox.querySelector("svg")).toHaveClass("size-[18px]", "shrink-0");
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
        drafts={[]}
        onOpenDraft={vi.fn()}
        onDeleteDraft={vi.fn()}
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

  it("places receive beside the folder heading and settings at the pane bottom", () => {
    const onReceive = vi.fn();
    const onOpenSettings = vi.fn();
    const { container } = render(
      <MailboxPane
        mailboxes={[]}
        selectedMailboxId=""
        onSelect={vi.fn()}
        onCompose={vi.fn()}
        drafts={[]}
        onOpenDraft={vi.fn()}
        onDeleteDraft={vi.fn()}
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
        drafts={[]}
        onOpenDraft={vi.fn()}
        onDeleteDraft={vi.fn()}
        onReceive={vi.fn()}
        receiving
        onOpenSettings={vi.fn()}
      />,
    );

    expect(within(container).getByRole("button", { name: "Receive" })).toBeDisabled();
  });
});
