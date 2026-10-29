import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { StrictMode } from "react";
import i18n from "@/app/i18n";
import { CompositionDefinitionEditorApp } from "./CompositionDefinitionEditorApp";

vi.mock("@/app/windowReady", () => ({ useRevealWindowWhenReady: vi.fn() }));
vi.mock("@/app/api", () => ({
  api: {
    getLastSelectedAccount: vi.fn().mockResolvedValue(null),
    listMailTemplates: vi.fn().mockResolvedValue([]),
    listMailSignatures: vi.fn().mockResolvedValue([]),
    listSystemFonts: vi.fn().mockResolvedValue([]),
  },
  normalizeCommandError: vi.fn(),
}));
beforeAll(async () => { await i18n.changeLanguage("en-US"); });
afterEach(cleanup);
describe("definition editor with the real rich text editor", () => {
  it.each(["template", "signature"] as const)("opens a new %s with an editable body", async kind => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(<StrictMode><QueryClientProvider client={client}><CompositionDefinitionEditorApp
      accountId={null} kind={kind} definitionId={null}
    /></QueryClientProvider></StrictMode>);
    expect(await screen.findByRole("toolbar")).toBeInTheDocument();
    expect(container.querySelector('[contenteditable="true"]')).toBeInTheDocument();
  });
});
