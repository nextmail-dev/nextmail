import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";

describe("Button", () => {
  it("uses theme depth without moving primary actions on hover", () => {
    render(<Button>Send</Button>);

    const button = screen.getByRole("button", { name: "Send" });
    expect(button).toHaveClass(
      "bg-primary",
      "[background:var(--primary-gradient)]",
      "shadow-[var(--shadow-primary)]",
      "hover:brightness-[1.04]",
      "cursor-default",
      "focus-visible:ring-1",
      "focus-visible:ring-inset",
    );
    expect(button.className).not.toMatch(/hover:-?translate/);
  });
});
