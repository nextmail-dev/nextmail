import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "../../app/i18n";
import type { AccountSummary } from "../../app/types";
import { AccountSwitcher } from "./AccountSwitcher";

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
    renderSwitcher([first]);
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Switch email account" })).not.toBeInTheDocument();
  });

  it("shows the account menu trigger only when multiple accounts exist", () => {
    renderSwitcher([first, second]);
    expect(screen.getByRole("button", { name: "Switch email account" })).toBeInTheDocument();
  });
});

function renderSwitcher(accounts: AccountSummary[]) {
  render(
    <AccountSwitcher
      accounts={accounts}
      selectedAccountId="one"
      onAccountChange={vi.fn()}
    />,
  );
}
