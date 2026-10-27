import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SwitchField } from "./switch";

afterEach(cleanup);

describe("SwitchField", () => {
  it("toggles once from the label, description or handle and exposes the current state", () => {
    const onChange = vi.fn();
    function Setting() {
      const [checked, setChecked] = useState(false);
      return <SwitchField label="Notifications" description="Show new mail" checked={checked} onCheckedChange={(next) => {
        onChange(next);
        setChecked(next);
      }} />;
    }
    render(<Setting />);
    const toggle = screen.getByRole("switch", { name: "Notifications" });
    expect(toggle).toHaveAccessibleDescription("Show new mail");
    expect(toggle).not.toBeChecked();
    fireEvent.click(screen.getByText("Notifications"));
    expect(toggle).toBeChecked();
    fireEvent.click(screen.getByText("Show new mail"));
    expect(toggle).not.toBeChecked();
    fireEvent.click(toggle);
    expect(toggle).toBeChecked();
    expect(onChange.mock.calls).toEqual([[true], [false], [true]]);
  });

  it("blocks label and handle activation while disabled and never submits a form", () => {
    const onChange = vi.fn();
    const onSubmit = vi.fn((event) => event.preventDefault());
    render(<form onSubmit={onSubmit}><SwitchField label="Notifications" description="Show new mail" disabled checked={false} onCheckedChange={onChange} /></form>);
    fireEvent.click(screen.getByText("Show new mail"));
    const toggle = screen.getByRole("switch");
    fireEvent.click(toggle);
    expect(onChange).not.toHaveBeenCalled();
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAttribute("type", "button");
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
