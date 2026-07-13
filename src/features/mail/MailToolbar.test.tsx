import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "../../app/i18n";
import type { AccountSummary } from "../../app/types";
import { AccountSwitcher } from "./AccountSwitcher";
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

describe("AccountSwitcher", () => {
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

describe("MailToolbar", () => {
  it("enables message actions only when a message is selected", () => {
    const { rerender } = render(<MailToolbar {...toolbarProps} selectedMessageId="" />);
    expect(screen.getByRole("button", { name: /^Reply$/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Forward" })).toBeDisabled();

    rerender(<MailToolbar {...toolbarProps} selectedMessageId="message-one" />);
    expect(screen.getByRole("button", { name: /^Reply$/ })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Forward" })).toBeEnabled();
  });
});

const toolbarProps = {
  onOpenSettings: vi.fn(),
  onOpenAccounts: vi.fn(),
  onOpenAbout: vi.fn(),
  onQuit: vi.fn(),
  searchQuery: "",
  onSearchChange: vi.fn(),
  onReceive: vi.fn(),
  receiving: false,
  selectedMailboxId: "inbox",
  mailboxes: [],
  activeMessageAction: null,
  onMessageAction: vi.fn(),
  onCopy: vi.fn(),
};

function renderToolbar(accounts: AccountSummary[]) {
  render(
    <AccountSwitcher
      accounts={accounts}
      selectedAccountId="one"
      onAccountChange={vi.fn()}
    />,
  );
}
