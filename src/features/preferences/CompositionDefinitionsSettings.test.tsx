import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import type { DraftContent, MailSignature, MailTemplate } from "@/app/types";
import { CompositionDefinitionsSettings } from "./CompositionDefinitionsSettings";

vi.mock("@/app/api", () => ({
  api: {
    createMailSignature: vi.fn(),
    createMailTemplate: vi.fn(),
    deleteMailSignature: vi.fn(),
    deleteMailTemplate: vi.fn(),
    getSignaturePreferences: vi.fn(),
    listMailSignatures: vi.fn(),
    listMailTemplates: vi.fn(),
    listCompositionSceneRules: vi.fn(),
    saveCompositionSceneRule: vi.fn(),
    saveSignaturePreferences: vi.fn(),
    updateMailSignature: vi.fn(),
    updateMailTemplate: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));

vi.mock("@/features/composer/RichTextEditor", () => ({
  RichTextEditor: ({ onChange }: { onChange: (content: DraftContent) => void }) => (
    <button
      type="button"
      onClick={() => onChange({
        editorJson: '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Reusable body"}]}]}',
        html: "<p>Reusable body</p>",
        plainText: "Reusable body",
      })}
    >
      Change rich text
    </button>
  ),
}));

const accounts = [
  { id: "account-one", email: "alice@example.com", displayName: "Alice" },
];

const existingTemplate: MailTemplate = {
  id: "template-one",
  scope: "global",
  accountId: null,
  name: "Follow up",
  subject: "Next steps",
  content: {
    editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
    html: "<p>Existing body</p>",
    plainText: "Existing body",
  },
  revision: 3,
  updatedAt: 1,
};

const signatures: MailSignature[] = [
  {
    id: "signature-one",
    scope: "global",
    accountId: null,
    name: "Primary",
    content: {
      editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
      html: "<p>Alice</p>",
      plainText: "Alice",
    },
    revision: 1,
    updatedAt: 1,
  },
  {
    id: "signature-two",
    scope: "global",
    accountId: null,
    name: "Compact",
    content: {
      editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
      html: "<p>A</p>",
      plainText: "A",
    },
    revision: 1,
    updatedAt: 2,
  },
];

function renderSettings() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return render(<CompositionDefinitionsSettings accounts={accounts} />, { wrapper: Wrapper });
}

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.listMailTemplates).mockResolvedValue([]);
  vi.mocked(api.listMailSignatures).mockResolvedValue([]);
  vi.mocked(api.getSignaturePreferences).mockResolvedValue({
    defaultSignatureId: null,
    autoInsert: true,
    inherited: false,
    revision: 0,
  });
  vi.mocked(api.listCompositionSceneRules).mockResolvedValue([
    { scene: "new", templateId: null, signatureId: null, inherited: false, revision: 0 },
    { scene: "reply", templateId: null, signatureId: null, inherited: false, revision: 0 },
    { scene: "reply_all", templateId: null, signatureId: null, inherited: false, revision: 0 },
    { scene: "forward", templateId: null, signatureId: null, inherited: false, revision: 0 },
  ]);
  vi.mocked(api.saveCompositionSceneRule).mockImplementation(async (_accountId, draft) => ({
    scene: draft.scene,
    templateId: draft.templateId,
    signatureId: draft.signatureId,
    inherited: false,
    revision: 1,
  }));
  vi.mocked(api.saveSignaturePreferences).mockImplementation(async (_accountId, draft) => ({
    defaultSignatureId: draft.defaultSignatureId,
    autoInsert: draft.autoInsert,
    inherited: false,
    revision: 2,
  }));
  vi.mocked(api.createMailTemplate).mockImplementation(async (_accountId, draft) => ({
    id: "template-new",
    scope: "global",
    accountId: null,
    revision: 1,
    updatedAt: 1,
    ...draft,
  }));
  vi.mocked(api.deleteMailTemplate).mockResolvedValue(undefined);
});

afterEach(cleanup);

describe("CompositionDefinitionsSettings", () => {
  it("keeps definition fields content-sized so the editor receives the remaining height", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "Add signature" }));
    const name = screen.getByRole("textbox", { name: "Name" });

    expect(name.parentElement?.parentElement).toHaveClass("flex-none");
  });

  it("creates a rich-text template in the explicit global scope", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "Add template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Welcome" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Subject" }), { target: { value: "Hello" } });
    fireEvent.click(screen.getByRole("button", { name: "Change rich text" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(api.createMailTemplate).toHaveBeenCalledWith(null, {
      name: "Welcome",
      subject: "Hello",
      content: {
        editorJson: '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Reusable body"}]}]}',
        html: "<p>Reusable body</p>",
        plainText: "Reusable body",
      },
    }));
  });

  it("reloads both libraries when the user selects an account scope", async () => {
    renderSettings();
    const scope = await screen.findByRole("combobox", { name: "Library scope" });

    fireEvent.pointerDown(scope, { button: 0, ctrlKey: false, pointerType: "mouse" });
    fireEvent.click(await screen.findByRole("option", { name: "Account: Alice" }));

    await waitFor(() => {
      expect(api.listMailTemplates).toHaveBeenCalledWith("account-one");
      expect(api.listMailSignatures).toHaveBeenCalledWith("account-one");
    });
  });

  it("requires an explicit second click before deleting a definition", async () => {
    vi.mocked(api.listMailTemplates).mockResolvedValue([existingTemplate]);
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    expect(api.deleteMailTemplate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Click again to delete" }));

    await waitFor(() => expect(api.deleteMailTemplate).toHaveBeenCalledWith(
      null,
      existingTemplate.id,
      existingTemplate.revision,
    ));
  });

  it("saves a separate default rule for the new-message scene", async () => {
    vi.mocked(api.listMailTemplates).mockResolvedValue([existingTemplate]);
    renderSettings();
    await screen.findByText("Follow up");
    const templateSelectors = await screen.findAllByRole("combobox", { name: "Template" });

    fireEvent.pointerDown(templateSelectors[0], { button: 0, ctrlKey: false, pointerType: "mouse" });
    fireEvent.click(await screen.findByRole("option", { name: "Follow up (Global)" }));

    await waitFor(() => expect(api.saveCompositionSceneRule).toHaveBeenCalledWith(
      null,
      {
        scene: "new",
        templateId: existingTemplate.id,
        signatureId: null,
        inherit: false,
      },
      0,
    ));
  });

  it("marks the default signature and can select another one", async () => {
    vi.mocked(api.listMailSignatures).mockResolvedValue(signatures);
    vi.mocked(api.getSignaturePreferences).mockResolvedValue({
      defaultSignatureId: signatures[0].id,
      autoInsert: true,
      inherited: false,
      revision: 4,
    });
    renderSettings();

    expect(await screen.findByText("Default")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Set as default" }));

    await waitFor(() => expect(api.saveSignaturePreferences).toHaveBeenCalledWith(
      null,
      {
        defaultSignatureId: signatures[1].id,
        autoInsert: true,
        inherit: false,
      },
      4,
    ));
  });

  it("toggles automatic default signature selection", async () => {
    vi.mocked(api.listMailSignatures).mockResolvedValue(signatures);
    vi.mocked(api.getSignaturePreferences).mockResolvedValue({
      defaultSignatureId: signatures[0].id,
      autoInsert: true,
      inherited: false,
      revision: 7,
    });
    renderSettings();

    await screen.findByText("Current default signature: Primary.");
    fireEvent.click(await screen.findByRole("checkbox", {
      name: "Automatically select the default signature for messages",
    }));

    await waitFor(() => expect(api.saveSignaturePreferences).toHaveBeenCalledWith(
      null,
      {
        defaultSignatureId: signatures[0].id,
        autoInsert: false,
        inherit: false,
      },
      7,
    ));
  });
});
