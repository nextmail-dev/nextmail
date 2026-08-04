import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import { ContactIdentity } from "./ContactIdentity";

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

describe("ContactIdentity", () => {
  it("waits before showing a pointer-hover card but opens immediately for keyboard focus", () => {
    vi.useFakeTimers();
    render(<ContactIdentity address={{ name: "Alice", email: "alice@example.com" }} tag />);
    const identity = screen.getByLabelText("Alice <alice@example.com>");

    fireEvent.mouseEnter(identity);
    act(() => vi.advanceTimersByTime(449));
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    fireEvent.mouseLeave(identity);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    fireEvent.focus(identity);
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    expect(identity).toHaveClass("bg-muted/55", "cursor-pointer");
  });

  it("copies identity fields and can open the linked local contact", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const onOpenContact = vi.fn();
    const onEditContact = vi.fn();
    render(
      <ContactIdentity
        address={{
          contactId: "contact-one",
          name: "Alice Local",
          headerName: "Header Alice",
          email: "alice@example.com",
        }}
        onOpenContact={onOpenContact}
        onEditContact={onEditContact}
      />,
    );

    const identity = screen.getByLabelText("Alice Local <alice@example.com>");
    fireEvent.contextMenu(identity);
    fireEvent.click(await screen.findByText("Copy email address"));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("alice@example.com"));

    fireEvent.contextMenu(identity);
    fireEvent.click(await screen.findByText("Open contact"));
    expect(onOpenContact).toHaveBeenCalledWith("contact-one");

    fireEvent.contextMenu(identity);
    fireEvent.click(await screen.findByText("Edit contact"));
    await waitFor(() => expect(onEditContact).toHaveBeenCalledWith("contact-one"));
  });
});
