import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Toast } from "./toast";

describe("Toast", () => {
  it("renders above the workspace below the custom titlebar", () => {
    const onClose = vi.fn();
    render(
      <Toast
        title="Message sent"
        description="Subject"
        closeLabel="Close"
        onClose={onClose}
      />,
    );

    const toast = screen.getByRole("status");
    expect(toast.parentElement).toBe(document.body);
    expect(toast).toHaveClass(
      "top-[calc(var(--titlebar-height)+0.75rem)]",
      "z-[120]",
    );

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
