import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Modal } from "./dialog";

describe("Modal", () => {
  it("renders above the custom window titlebar", () => {
    render(
      <Modal open onOpenChange={vi.fn()} title="Confirm" closeLabel="Close">
        Content
      </Modal>,
    );

    expect(screen.getByRole("dialog", { name: "Confirm" })).toHaveClass("app-dialog-content");
    expect(document.querySelector("[data-state='open'].fixed.inset-0")).toHaveClass("app-dialog-overlay");
  });
});
