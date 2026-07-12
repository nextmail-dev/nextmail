import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SafeMailFrame } from "./SafeMailFrame";

describe("SafeMailFrame", () => {
  it("renders mail HTML in a scriptless isolated frame", () => {
    render(<SafeMailFrame document="<p>Hello</p>" title="Message" />);

    const frame = screen.getByTitle("Message");
    expect(frame).toHaveAttribute("sandbox", "");
    expect(frame).toHaveAttribute("referrerpolicy", "no-referrer");
    expect(frame).not.toHaveAttribute("allow");
    expect(frame).toHaveAttribute("srcdoc", "<p>Hello</p>");
  });
});
