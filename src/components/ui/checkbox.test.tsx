import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Checkbox } from "./checkbox";

describe("Checkbox", () => {
  it("uses the description as part of the full clickable preference row", () => {
    const onCheckedChange = vi.fn();
    render(
      <Checkbox
        checked={false}
        label="Automatic updates"
        description="Check when NextMail starts."
        onCheckedChange={onCheckedChange}
      />,
    );

    const row = screen.getByText("Automatic updates").closest("label");
    expect(row).toHaveClass("w-full", "max-w-full", "rounded-md", "hover:bg-accent");
    fireEvent.click(screen.getByText("Check when NextMail starts."));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });
});
