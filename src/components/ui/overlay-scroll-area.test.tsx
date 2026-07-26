import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OverlayScrollArea } from "./overlay-scroll-area";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("OverlayScrollArea", () => {
  it("keeps a draggable custom thumb visible whenever the viewport can scroll", () => {
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(100);
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(400);

    const { container } = render(
      <OverlayScrollArea trackClassName="right-2 w-3">
        <p>Scrollable content</p>
      </OverlayScrollArea>,
    );

    const viewport = container.querySelector(".native-scrollbar-hidden");
    const track = container.querySelector('[aria-hidden="true"]');
    const thumb = track?.firstElementChild;
    expect(viewport).toBeInTheDocument();
    expect(container.firstElementChild).toHaveAttribute("data-scrollbar-auto-hide", "false");
    expect(track).toHaveClass("right-2", "w-3");
    expect(thumb).toHaveClass("pointer-events-auto", "w-full");
    expect(thumb).not.toHaveClass("opacity-0", "pointer-events-none");
  });

  it("only hides the custom thumb when the caller explicitly requests it", () => {
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(100);
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(400);

    const { container } = render(
      <OverlayScrollArea autoHide>
        <p>Folder list</p>
      </OverlayScrollArea>,
    );

    const track = container.querySelector('[aria-hidden="true"]');
    expect(container.firstElementChild).toHaveAttribute("data-scrollbar-auto-hide", "true");
    expect(track).toHaveClass(
      "opacity-0",
      "group-hover/scroll-area:opacity-100",
      "group-focus-within/scroll-area:opacity-100",
    );
    expect(track?.firstElementChild).toHaveClass("pointer-events-none");
  });
});
