import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "../../app/i18n";
import type { AccountSummary } from "../../app/types";
import { MailToolbar } from "./MailToolbar";

const first: AccountSummary = {
  id: "one",
  email: "alice@example.com",
  displayName: "Alice",
};
const second: AccountSummary = {
  id: "two",
  email: "bob@example.com",
  displayName: "Bob",
};

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("MailToolbar", () => {
  it("shows a static identity for a single account", () => {
    renderToolbar([first]);

    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Switch email account" })).not.toBeInTheDocument();
  });

  it("shows the account menu trigger only when multiple accounts exist", () => {
    renderToolbar([first, second]);

    expect(screen.getByRole("button", { name: "Switch email account" })).toBeInTheDocument();
  });
});

function renderToolbar(accounts: AccountSummary[]) {
  render(
    <MailToolbar
      accounts={accounts}
      selectedAccountId="one"
      onAccountChange={vi.fn()}
      onCompose={vi.fn()}
      drafts={[]}
      onOpenDraft={vi.fn()}
      onOpenSettings={vi.fn()}
      onOpenAccounts={vi.fn()}
      onOpenAbout={vi.fn()}
      onQuit={vi.fn()}
    />,
  );
}
