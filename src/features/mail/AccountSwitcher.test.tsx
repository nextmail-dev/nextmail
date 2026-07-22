import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

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

afterEach(cleanup);

describe("AccountSwitcher", () => {
  it("keeps the account menu available for a single account", async () => {
    const onManageAccounts = vi.fn();
    renderSwitcher([first], onManageAccounts);
    expect(screen.getByText("Alice")).toBeInTheDocument();
    openAccountMenu();
    fireEvent.click(await screen.findByRole("menuitem", { name: "Account management" }));
    expect(onManageAccounts).toHaveBeenCalledOnce();
  });

  it("shows account-local runtime state and switches to the selected account", async () => {
    const onAccountChange = vi.fn();
    render(
      <AccountSwitcher
        accounts={[first, second]}
        selectedAccountId="one"
        onAccountChange={onAccountChange}
        onManageAccounts={vi.fn()}
        runtimeSummaries={[
          { accountId: "one", state: "ready", errorCode: null, retryAt: null, revision: 1 },
          { accountId: "two", state: "reauth_required", errorCode: "credential.read_failed", retryAt: null, revision: 2 },
        ]}
      />,
    );

    openAccountMenu();
    const runtimeLabel = await screen.findByText(/bob@example\.com · Reauthentication required/);
    expect(runtimeLabel.closest('[role="menuitemcheckbox"]')).toHaveClass("min-h-14");
    expect(screen.getByText("Bob")).toHaveClass("text-left");
    fireEvent.click(runtimeLabel);
    expect(onAccountChange).toHaveBeenCalledWith("two");
  });

  it("does not add synchronization status text to the account identity", () => {
    render(
      <AccountSwitcher
        accounts={[first]}
        selectedAccountId="one"
        onAccountChange={vi.fn()}
        onManageAccounts={vi.fn()}
        runtimeSummaries={[
          { accountId: "one", state: "syncing", errorCode: null, retryAt: null, revision: 2 },
        ]}
      />,
    );

    expect(screen.getByText("alice@example.com")).toBeInTheDocument();
    expect(screen.queryByText("Synchronizing")).not.toBeInTheDocument();
  });
});

function renderSwitcher(accounts: AccountSummary[], onManageAccounts = vi.fn()) {
  render(
    <AccountSwitcher
      accounts={accounts}
      selectedAccountId="one"
      onAccountChange={vi.fn()}
      onManageAccounts={onManageAccounts}
    />,
  );
}

function openAccountMenu() {
  fireEvent.pointerDown(screen.getByRole("button", { name: "Open account menu" }), {
    button: 0,
    ctrlKey: false,
  });
}
