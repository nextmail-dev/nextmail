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
    expect(frame.getAttribute("srcdoc")).toContain("background:#fbfcfe");
    expect(frame.getAttribute("srcdoc")).toContain("background-color: rgb(251, 252, 254) !important");
    expect(frame.getAttribute("srcdoc")).toContain("Hello Taylor");
  });

  it("places light reader defaults before authored non-white backgrounds", () => {
    const source = "<!doctype html><html><head><style>body{background:#fefefe}</style></head><body>Mail</body></html>";
    render(<SafeMailFrame document={source} title="Tinted" />);

    const frameSource = screen.getByTitle("Tinted").getAttribute("srcdoc") ?? "";
    expect(frameSource.indexOf('id="nextmail-reader-theme"')).toBeLessThan(
      frameSource.indexOf("body{background:#fefefe}"),
    );
    expect(new DOMParser().parseFromString(frameSource, "text/html").body.style.backgroundColor).toBe("");
  });

  it("only enables remote image sources after explicit approval and adapts dark mail bodies", () => {
    document.documentElement.dataset.theme = "dark";
    const source = '<meta http-equiv="Content-Security-Policy" content="img-src data:;"><p>Mail</p>';
    render(<SafeMailFrame document={source} title="Remote" allowRemoteImages />);
    const frame = screen.getByTitle("Remote");
    expect(frame).toHaveStyle({ colorScheme: "dark" });
    expect(frame.getAttribute("srcdoc")).toContain("img-src data: http: https:;");
    expect(frame.getAttribute("srcdoc")).toContain("background:#171717");
    expect(frame.getAttribute("srcdoc")).toContain("color:#e8e8e8");
    expect(frame.getAttribute("srcdoc")).toContain("*{border-color:#6f6f6f}");
    expect(frame.getAttribute("srcdoc")).toContain("!important");
    document.documentElement.removeAttribute("data-theme");
  });

  it("places reader defaults before authored styles and writes computed dark colors inline", () => {
    document.documentElement.dataset.theme = "dark";
    const source = "<!doctype html><html><head></head><body><style>body{background:#fff;color:#111}</style><p>Authored</p></body></html>";
    render(<SafeMailFrame document={source} title="Authored colors" />);

    const frameSource = screen.getByTitle("Authored colors").getAttribute("srcdoc") ?? "";
    expect(frameSource.indexOf('id="nextmail-reader-theme"')).toBeLessThan(
      frameSource.indexOf("body{background:#fff;color:#111}"),
    );
    expect(frameSource).toContain("background-color: rgb(");
    expect(frameSource).toContain("!important");
    document.documentElement.removeAttribute("data-theme");
  });

  it("trusts sanitized native-dark mail without applying smart inversion twice", () => {
    document.documentElement.dataset.theme = "dark";
    const source = '<!doctype html><html data-nextmail-native-dark=""><head></head><body style="background-color:#252525;color:#eeeeee">Native</body></html>';
    render(<SafeMailFrame document={source} title="Native dark" />);

    const frameSource = screen.getByTitle("Native dark").getAttribute("srcdoc") ?? "";
    expect(frameSource).toContain('data-nextmail-native-dark=""');
    expect(frameSource).toContain('style="background-color:#252525;color:#eeeeee"');
    expect(frameSource).not.toContain("background-color: rgb(");
    document.documentElement.removeAttribute("data-theme");
  });

  it("constrains message images to the frame viewport", () => {
    const source = '<!doctype html><html><head></head><body><img src="data:image/png;base64,AA==" width="1200" height="800" alt="Chart"><img src="https://cdn.example/banner.png"></body></html>';
    render(<SafeMailFrame document={source} title="Images" />);

    const frame = screen.getByTitle("Images") as HTMLIFrameElement;
    const frameDocument = new DOMParser().parseFromString(
      frame.getAttribute("srcdoc") ?? "",
      "text/html",
    );
    const images = frameDocument.querySelectorAll("img");
    expect(images[0].style.getPropertyValue("max-width")).toBe("calc(100vw - 16px)");
    expect(images[0].style.getPropertyPriority("max-width")).toBe("important");
    expect(images[0].style.getPropertyValue("box-sizing")).toBe("border-box");
    expect(images[0].style.getPropertyValue("height")).toBe("auto");
  });

  it("keeps a themed scrollbar visible inside the reader inset without hover", () => {
    document.documentElement.style.setProperty("--muted-foreground", "#5f6d80");
    try {
      render(<SafeMailFrame document={plainUnstyledMail} title="Scrollbars" />);
      const source = screen.getByTitle("Scrollbars").getAttribute("srcdoc") ?? "";
      const frameDocument = new DOMParser().parseFromString(source, "text/html");
      const styles = frameDocument.getElementById("nextmail-reader-scrollbar")?.textContent ?? "";
      expect(styles).toContain("width:calc(100vw - 16px) !important");
      expect(styles).toContain("margin:0 0 0 8px !important");
      expect(styles).toMatch(/:root::-webkit-scrollbar\s*\{[^}]*width:6px/);
      expect(styles).toMatch(/:root::-webkit-scrollbar-thumb\s*\{[^}]*background:color-mix\(in srgb, #5f6d80 55%, transparent\)/);
      expect(styles).toContain("cursor:default");
      expect(styles).not.toContain(":root:hover");
      expect(styles).not.toContain("scrollbar-gutter");
      expect(frameDocument.querySelector("script")).toBeNull();
    } finally {
      document.documentElement.style.removeProperty("--muted-foreground");
    }
  });
});
