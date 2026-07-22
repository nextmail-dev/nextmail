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
  it("shows a static identity for a single account", () => {
    renderSwitcher([first]);
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Switch email account" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "NextMail menu" })).not.toBeInTheDocument();
  });

  it("shows the account menu trigger only when multiple accounts exist", () => {
    renderSwitcher([first, second]);
    expect(screen.getByRole("button", { name: "Switch email account" })).toBeInTheDocument();
  });

  it("shows account-local runtime state and switches to the selected account", async () => {
    const onAccountChange = vi.fn();
    render(
      <AccountSwitcher
        accounts={[first, second]}
        selectedAccountId="one"
        onAccountChange={onAccountChange}
        runtimeSummaries={[
          { accountId: "one", state: "ready", errorCode: null, retryAt: null, revision: 1 },
          { accountId: "two", state: "reauth_required", errorCode: "credential.read_failed", retryAt: null, revision: 2 },
        ]}
      />,
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: "Switch email account" }), {
      button: 0,
      ctrlKey: false,
    });
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
        runtimeSummaries={[
          { accountId: "one", state: "syncing", errorCode: null, retryAt: null, revision: 2 },
        ]}
      />,
    );

    expect(screen.getByText("alice@example.com")).toBeInTheDocument();
    expect(screen.queryByText("Synchronizing")).not.toBeInTheDocument();
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
