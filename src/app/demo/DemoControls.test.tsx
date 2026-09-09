import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "../i18n";
import { api } from "../api";
import { DemoEntryTitle, DemoLanguageDialog } from "./DemoControls";
import { isDemoMode } from "./session";

vi.mock("./session", () => ({ isDemoMode: vi.fn(() => true) }));
vi.mock("../api", () => ({
  api: {
    enterDemoMode: vi.fn().mockResolvedValue(undefined),
    getPreferences: vi.fn().mockResolvedValue({ theme: "light", accentColor: "#2563eb", language: "en-US" }),
    setAppearancePreferences: vi.fn().mockImplementation(async (value) => value),
  },
  normalizeCommandError: (error: unknown) => error,
}));

beforeEach(async () => { vi.clearAllMocks(); await i18n.changeLanguage("en-US"); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe("hidden demo controls", () => {
  it("requires eleven consecutive clicks and an explicit confirmation", async () => {
    render(<DemoEntryTitle />);
    const title = screen.getByRole("button", { name: "NextMail" });
    for (let count = 0; count < 10; count++) fireEvent.click(title);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(title);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(api.enterDemoMode).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.enterDemoMode).not.toHaveBeenCalled();
    for (let count = 0; count < 11; count++) fireEvent.click(title);
    fireEvent.click(screen.getByRole("button", { name: "Enter demo mode" }));
    await waitFor(() => expect(api.enterDemoMode).toHaveBeenCalledTimes(1));
  });

  it("resets the sequence on a pause or a click elsewhere", () => {
    let now = 5000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    render(<DemoEntryTitle />);
    const title = screen.getByRole("button", { name: "NextMail" });
    for (let count = 0; count < 10; count++) fireEvent.click(title);
    now += 2001;
    fireEvent.click(title);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    for (let count = 0; count < 9; count++) fireEvent.click(title);
    fireEvent.click(document.body);
    fireEvent.click(title);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens from focused inputs with Ctrl+L and applies the chosen language", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><input aria-label="Search" /><DemoLanguageDialog /></QueryClientProvider>);
    await waitFor(() => expect(client.getQueryData(["preferences"])).toBeDefined());
    const input = screen.getByRole("textbox");
    input.focus();
    fireEvent.keyDown(input, { key: "l", ctrlKey: true });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "简体中文" }));
    await waitFor(() => expect(api.setAppearancePreferences).toHaveBeenCalledWith(expect.objectContaining({ language: "zh-CN" }), expect.anything()));
  });

  it("does not reserve the shortcut outside demo mode", () => {
    vi.mocked(isDemoMode).mockReturnValueOnce(false);
    render(<QueryClientProvider client={new QueryClient()}><DemoLanguageDialog /></QueryClientProvider>);
    fireEvent.keyDown(window, { key: "l", ctrlKey: true });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
