import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import plainUnstyledMail from "../../../testdata/mail-rendering/plain-unstyled.html?raw";

import { SafeMailFrame } from "./SafeMailFrame";

describe("SafeMailFrame", () => {
  it("renders mail HTML in a scriptless isolated frame", () => {
    render(<SafeMailFrame document={plainUnstyledMail} title="Message" />);

    const frame = screen.getByTitle("Message");
    expect(frame).toHaveAttribute("sandbox", "allow-popups");
    for (const forbidden of ["allow-scripts", "allow-forms", "allow-same-origin", "allow-top-navigation"]) {
      expect(frame.getAttribute("sandbox")).not.toContain(forbidden);
    }
    expect(frame).toHaveAttribute("referrerpolicy", "no-referrer");
    expect(frame).not.toHaveAttribute("allow");
    expect(frame).toHaveStyle({ colorScheme: "light" });
    expect(frame.getAttribute("srcdoc")).toContain("background:#fff");
    expect(frame.getAttribute("srcdoc")).not.toContain("!important");
    expect(frame.getAttribute("srcdoc")).toContain("Hello Taylor");
  });

  it("only enables remote image sources after explicit approval and adapts dark mail bodies", () => {
    document.documentElement.dataset.theme = "dark";
    const source = '<meta http-equiv="Content-Security-Policy" content="img-src data:;"><p>Mail</p>';
    render(<SafeMailFrame document={source} title="Remote" allowRemoteImages />);
    const frame = screen.getByTitle("Remote");
    expect(frame).toHaveStyle({ colorScheme: "dark" });
    expect(frame.getAttribute("srcdoc")).toContain("img-src data: http: https:;");
    expect(frame.getAttribute("srcdoc")).toContain("background:#181818");
    expect(frame.getAttribute("srcdoc")).toContain("color:#e8e8e8");
    expect(frame.getAttribute("srcdoc")).not.toContain("!important");
    document.documentElement.removeAttribute("data-theme");
  });

  it("places reader defaults before authored body styles so explicit mail colors win", () => {
    document.documentElement.dataset.theme = "dark";
    const source = "<!doctype html><html><head></head><body><style>body{background:#fff;color:#111}</style><p>Authored</p></body></html>";
    render(<SafeMailFrame document={source} title="Authored colors" />);

    const frameSource = screen.getByTitle("Authored colors").getAttribute("srcdoc") ?? "";
    expect(frameSource.indexOf('id="nextmail-reader-theme"')).toBeLessThan(
      frameSource.indexOf("body{background:#fff;color:#111}"),
    );
    expect(frameSource).not.toContain("!important");
    document.documentElement.removeAttribute("data-theme");
  });
});
