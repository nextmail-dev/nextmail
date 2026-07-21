import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import plainUnstyledMail from "../../../testdata/mail-rendering/plain-unstyled.html?raw";

import { SafeMailFrame } from "./SafeMailFrame";

describe("SafeMailFrame", () => {
  it("renders mail HTML in a scriptless isolated frame", () => {
    render(<SafeMailFrame document={plainUnstyledMail} title="Message" />);

    const frame = screen.getByTitle("Message");
    expect(frame).toHaveAttribute("sandbox", "");
    expect(frame).toHaveAttribute("referrerpolicy", "no-referrer");
    expect(frame).not.toHaveAttribute("allow");
    expect(frame.getAttribute("srcdoc")).toContain("background:#fff");
    expect(frame.getAttribute("srcdoc")).toContain("Hello Taylor");
  });

  it("only enables remote image sources after explicit approval and adapts dark mail bodies", () => {
    document.documentElement.dataset.theme = "dark";
    const source = '<meta http-equiv="Content-Security-Policy" content="img-src data:;"><p>Mail</p>';
    render(<SafeMailFrame document={source} title="Remote" allowRemoteImages />);
    const frame = screen.getByTitle("Remote");
    expect(frame.getAttribute("srcdoc")).toContain("img-src data: http: https:;");
    expect(frame.getAttribute("srcdoc")).toContain("background:#181818");
    expect(frame.getAttribute("srcdoc")).not.toContain("background-color:transparent!important");
    document.documentElement.removeAttribute("data-theme");
  });
});
