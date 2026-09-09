import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../api";
import i18n from "../i18n";
import { MainShell } from "@/features/mail/MainShell";
import { initializeDemoSession } from "./session";
import content from "./messages.json";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }));
afterEach(() => { cleanup(); Reflect.deleteProperty(globalThis, "__TAURI_INTERNALS__"); });

it("renders the existing mail shell entirely from demo data, including the sandbox reader", async () => {
  await i18n.changeLanguage("en-US");
  Object.defineProperty(globalThis, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  vi.mocked(invoke).mockResolvedValueOnce({ theme: "light", accentColor: "#2563eb", language: "en-US" }).mockResolvedValueOnce(content);
  await initializeDemoSession();
  vi.mocked(invoke).mockClear();
  const bootstrap = await api.getBootstrapStatus();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><MainShell accounts={bootstrap.accounts} lastSelectedAccountId={bootstrap.lastSelectedAccountId} /></QueryClientProvider>);
  expect(await screen.findByText("NextMail Business")).toBeInTheDocument();
  const compose = screen.getByRole("button", { name: "New message" });
  expect(compose).toBeEnabled();
  fireEvent.click(compose);
  expect(invoke).not.toHaveBeenCalled();
  const subject = "Coastline Journal #42: Make room for curiosity";
  fireEvent.click(await screen.findByText(subject));
  const frame = await screen.findByTitle(subject);
  expect(frame).toHaveAttribute("sandbox", "allow-popups");
  expect(frame).toHaveAttribute("referrerpolicy", "no-referrer");
  expect(frame.getAttribute("srcdoc")).toContain("Make room for curiosity");
  expect(invoke).not.toHaveBeenCalled();

  fireEvent.click(screen.getByText("Unread"));
  const proposal = "Autumn brand proposal · Final review";
  fireEvent.click(await screen.findByText(proposal));
  await waitFor(async () => expect(await api.getMessageDetail("demo-account", "demo-message-0", "demo-inbox")).toMatchObject({ unread: false }));
  vi.mocked(invoke).mockResolvedValueOnce({ theme: "light", accentColor: "#2563eb", language: "zh-CN" });
  await act(async () => {
    await api.setAppearancePreferences({ theme: "light", accentColor: "#2563eb", language: "zh-CN" });
    await i18n.changeLanguage("zh-CN");
    await client.invalidateQueries();
  });
  await waitFor(() => expect(screen.queryByText(proposal)).not.toBeInTheDocument());
  expect(screen.getAllByText("秋季品牌提案 · 最终评审安排")).toHaveLength(2);
});
