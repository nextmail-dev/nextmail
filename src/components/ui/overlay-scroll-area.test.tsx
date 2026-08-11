import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OverlayScrollArea } from "./overlay-scroll-area";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("OverlayScrollArea", () => {
  it("uses an overlay thumb that only appears on hover or keyboard focus", () => {
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
    expect(viewport).not.toHaveClass("pr-4");
    expect(container.firstElementChild).toHaveAttribute("data-scrollbar-auto-hide", "true");
    expect(track).toHaveClass("right-2", "w-3");
    expect(track).toHaveClass(
      "opacity-0",
      "group-hover/scroll-area:opacity-100",
      "group-focus-within/scroll-area:opacity-100",
    );
    expect(thumb).toHaveClass("pointer-events-none", "w-full", "cursor-default");
    expect(thumb?.firstElementChild).toHaveClass("w-1.5");
  });
});
