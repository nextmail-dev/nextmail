import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SelectField } from "./select";

describe("SelectField", () => {
  it("keeps long option lists within the available viewport and scrollable", async () => {
    render(
      <SelectField
        label="Folder"
        value="option-1"
        options={Array.from({ length: 40 }, (_, index) => ({
          value: `option-${index + 1}`,
          label: `Option ${index + 1}`,
        }))}
        onValueChange={vi.fn()}
      />,
    );

    fireEvent.pointerDown(screen.getByRole("combobox", { name: "Folder" }), {
      button: 0,
      ctrlKey: false,
      pointerType: "mouse",
    });

    const option = await screen.findByRole("option", { name: "Option 40" });
    expect(option.closest(".app-floating-content")).toHaveClass(
      "max-h-[var(--radix-select-content-available-height)]",
    );
    expect(option.parentElement).toHaveClass("app-select-viewport", "overflow-y-auto", "overscroll-contain");
  });
});
