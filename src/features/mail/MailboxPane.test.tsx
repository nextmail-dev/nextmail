import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "../../app/i18n";
import type { DraftListItem } from "../../app/types";
import { MailboxPane } from "./MailboxPane";

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
});

