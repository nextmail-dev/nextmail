import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ResizeHandle } from "./resize-handle";

describe("ResizeHandle", () => {
  it("keeps a theme-aware pane boundary visible and emphasizes it on interaction", () => {
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
      "bg-border-strong",
      "group-hover:w-[2px]",
      "group-hover:bg-primary",
      "group-focus-visible:w-[2px]",
      "group-focus-visible:bg-primary",
    );
  });
});
