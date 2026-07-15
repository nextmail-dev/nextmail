import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ResizeHandle } from "./resize-handle";

describe("ResizeHandle", () => {
  it("reveals a theme-aware drag rail on hover and keyboard focus", () => {
    render(
      <ResizeHandle
        value={320}
        min={220}
        max={520}
        onValueChange={vi.fn()}
        label="Resize pane"
      />,
    );

    const separator = screen.getByRole("separator", { name: "Resize pane" });
    const rail = separator.querySelector("span");
    expect(separator).toHaveClass("w-3", "-translate-x-1/2");
    expect(rail).toHaveClass(
      "inset-y-0",
      "w-px",
      "bg-foreground/20",
      "opacity-0",
      "group-hover:opacity-100",
      "group-focus-visible:opacity-100",
    );
  });
});
