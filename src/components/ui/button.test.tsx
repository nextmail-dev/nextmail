import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";

describe("Button", () => {
  it("keeps primary actions flat and keyboard focus visible", () => {
    render(<Button>Send</Button>);

    const button = screen.getByRole("button", { name: "Send" });
    expect(button).toHaveClass(
      "bg-primary",
      "hover:bg-primary/85",
      "rounded-md",
      "cursor-default",
      "focus-visible:ring-1",
      "focus-visible:ring-inset",
    );
    expect(button.className).not.toMatch(/hover:-?translate/);
  });
});
