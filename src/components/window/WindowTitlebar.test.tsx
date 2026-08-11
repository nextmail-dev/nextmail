import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import { WindowTitlebar } from "./WindowTitlebar";

const { platformState, windowMock } = vi.hoisted(() => ({
  platformState: { value: "windows" },
  windowMock: {
    close: vi.fn(),
    destroy: vi.fn(),
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
  },
}));

vi.mock("@/app/platform", () => ({
  detectDesktopPlatform: () => platformState.value,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowMock,
}));

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("WindowTitlebar", () => {
  it("renders custom controls and maximize gestures only on Windows", () => {
    platformState.value = "windows";
    const { container } = render(<WindowTitlebar kind="main" />);

    expect(screen.getByRole("navigation", { name: "Window controls" })).toBeInTheDocument();
    fireEvent.doubleClick(container.querySelector("header")!);
    expect(windowMock.toggleMaximize).toHaveBeenCalledOnce();
  });

  it("leaves macOS and other platforms to their native window controls", () => {
    platformState.value = "macos";
    const mac = render(<WindowTitlebar kind="settings" />);
    expect(mac.container.querySelector("header")).toHaveClass("window-titlebar--mac");
    expect(screen.queryByRole("navigation", { name: "Window controls" })).not.toBeInTheDocument();
    fireEvent.doubleClick(mac.container.querySelector("header")!);
    expect(windowMock.toggleMaximize).not.toHaveBeenCalled();

    mac.unmount();
    platformState.value = "other";
    render(<WindowTitlebar kind="settings" />);
    expect(screen.queryByRole("navigation", { name: "Window controls" })).not.toBeInTheDocument();
  });

  it("dims title content while the app window is inactive", () => {
    platformState.value = "windows";
    const { container } = render(<WindowTitlebar kind="main" />);
    const titlebar = container.querySelector("header")!;

    fireEvent.focus(window);
    expect(titlebar).not.toHaveClass("window-titlebar--inactive");
    fireEvent.blur(window);
    expect(titlebar).toHaveClass("window-titlebar--inactive");
  });
});
