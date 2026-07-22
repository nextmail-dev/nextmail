import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { RecipientField } from "./RecipientField";

afterEach(cleanup);

describe("RecipientField", () => {
  it("restores the last tag to the input for editing on Backspace", () => {
    const address = { name: "Alice", email: "alice@example.com" };
    const onEditLast = vi.fn();
    const onRemove = vi.fn();
    render(
      <RecipientField
        label="To"
        addresses={[address]}
        input=""
        onInputChange={vi.fn()}
        onCommit={vi.fn()}
        onRemove={onRemove}
        onEditLast={onEditLast}
      />,
    );

    fireEvent.keyDown(screen.getByRole("textbox", { name: "To" }), { key: "Backspace" });

    expect(onEditLast).toHaveBeenCalledWith(address, 0);
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("commits immediately when a delimiter is pressed", () => {
    const onCommit = vi.fn();
    render(
      <RecipientField
        label="To"
        addresses={[]}
        input="alice@example.com"
        onInputChange={vi.fn()}
        onCommit={onCommit}
        onRemove={vi.fn()}
        onEditLast={vi.fn()}
      />,
    );

    fireEvent.keyDown(screen.getByRole("textbox", { name: "To" }), { key: "," });
    expect(onCommit).toHaveBeenCalledOnce();
  });
});
