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
    expect(rail).toHaveClass(
      "bg-foreground/20",
      "opacity-0",
      "group-hover:opacity-100",
      "group-focus-visible:opacity-100",
    );
  });
});
