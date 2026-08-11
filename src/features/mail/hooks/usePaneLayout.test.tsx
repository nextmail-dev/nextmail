import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { usePaneLayout } from "./usePaneLayout";

const originalInnerWidth = window.innerWidth;

function setWindowWidth(width: number) {
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    value: width,
    writable: true,
  });
}

beforeEach(() => {
  setWindowWidth(1400);
});

afterEach(() => {
  setWindowWidth(originalInnerWidth);
});

describe("usePaneLayout", () => {
  it("clamps both resizable panes to their minimum and maximum widths", () => {
    const { result } = renderHook(() => usePaneLayout());

    act(() => result.current.setFolderPaneWidth(999));
    expect(result.current.folderPaneWidth).toBe(350);
    act(() => result.current.setMessagePaneWidth(999));
    expect(result.current.messagePaneWidth).toBe(520);

    act(() => result.current.setFolderPaneWidth(1));
    expect(result.current.folderPaneWidth).toBe(220);
    act(() => result.current.setMessagePaneWidth(1));
    expect(result.current.messagePaneWidth).toBe(310);
  });

  it("preserves the expanded width while collapsing", () => {
    const { result } = renderHook(() => usePaneLayout());

    act(() => result.current.setFolderPaneWidth(320));
    act(() => result.current.setFolderPaneCollapsed(true));

    expect(result.current.folderPaneCollapsed).toBe(true);
    expect(result.current.folderPaneWidth).toBe(320);
    expect(result.current.visibleFolderWidth).toBe(72);
    act(() => result.current.setFolderPaneCollapsed(false));
    expect(result.current.visibleFolderWidth).toBe(320);
  });

  it("uses the latest widths when a window resize requires both panes to shrink", () => {
    const { result } = renderHook(() => usePaneLayout());
    act(() => result.current.setFolderPaneWidth(350));
    act(() => result.current.setMessagePaneWidth(520));

    act(() => {
      setWindowWidth(1000);
      window.dispatchEvent(new Event("resize"));
    });

    expect(result.current.messagePaneWidth).toBe(310);
    expect(result.current.folderPaneWidth).toBe(318);
  });

  it("preserves the reader minimum at the supported minimum window width", () => {
    setWindowWidth(920);
    const { result } = renderHook(() => usePaneLayout());

    expect(result.current.folderPaneWidth).toBe(238);
    expect(result.current.messagePaneWidth).toBe(310);
    expect(920 - result.current.visibleFolderWidth - result.current.messagePaneWidth).toBe(372);
  });
});
