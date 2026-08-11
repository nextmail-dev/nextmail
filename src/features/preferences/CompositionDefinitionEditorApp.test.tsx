import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import type { DraftContent } from "@/app/types";
import { CompositionDefinitionEditorApp } from "./CompositionDefinitionEditorApp";

vi.mock("@/app/windowReady", () => ({ useRevealWindowWhenReady: vi.fn() }));

vi.mock("@/app/api", () => ({
  api: {
    createMailSignature: vi.fn(),
    createMailTemplate: vi.fn(),
    getLastSelectedAccount: vi.fn(),
    listContactSuggestions: vi.fn(),
    listMailSignatures: vi.fn(),
    listMailTemplates: vi.fn(),
    prepareCompositionDefinitionImage: vi.fn(),
    resolveContactAddresses: vi.fn(),
    sanitizeRichTextPaste: vi.fn(),
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
  RichTextEditor: ({
    onChange,
    onAddInlineImage,
  }: {
    onChange: (content: DraftContent) => void;
    onAddInlineImage?: (file: File) => Promise<{ previewDataUrl: string | null }>;
  }) => (
    <button
      type="button"
      onClick={async () => {
        const image = await onAddInlineImage?.(new File(
          [new Uint8Array([0x89, 0x50, 0x4e, 0x47])],
          "logo.png",
          { type: "image/png" },
        ));
        const source = image?.previewDataUrl ?? "";
        onChange({
          editorJson: JSON.stringify({
            type: "doc",
            content: [{ type: "paragraph", content: [{
              type: "nextmailImage",
              attrs: { src: source },
            }] }],
          }),
          html: `<p><img src="${source}"></p>`,
          plainText: "",
        });
      }}
    >
      Insert embedded image
    </button>
  ),
}));

function renderEditor(
  kind: "template" | "signature",
  definitionId: string | null = null,
  accountId: string | null = null,
) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return render(
    <CompositionDefinitionEditorApp
      accountId={accountId}
      kind={kind}
      definitionId={definitionId}
    />,
    { wrapper: Wrapper },
  );
}

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.listMailTemplates).mockResolvedValue([]);
  vi.mocked(api.listMailSignatures).mockResolvedValue([]);
  vi.mocked(api.getLastSelectedAccount).mockResolvedValue("account-one");
  vi.mocked(api.listContactSuggestions).mockResolvedValue([{
    id: "contact-one",
    name: "Alice Local",
    email: "alice@example.com",
    revision: 1,
    createdAt: 1,
    updatedAt: 1,
  }]);
  vi.mocked(api.resolveContactAddresses).mockImplementation(async (_accountId, addresses) => addresses.map((address) => ({
    contactId: address.email === "alice@example.com" ? "contact-one" : null,
    name: address.name,
    headerName: address.name,
    email: address.email,
  })));
  vi.mocked(api.prepareCompositionDefinitionImage).mockResolvedValue({
    fileName: "logo.png",
    contentType: "image/png",
    size: 4,
    dataUrl: "data:image/png;base64,iVBORw==",
  });
  vi.mocked(api.createMailSignature).mockImplementation(async (_accountId, draft) => ({
    id: "signature-new",
    scope: "global",
    accountId: null,
    revision: 1,
    updatedAt: 1,
    ...draft,
  }));
});

afterEach(cleanup);

describe("CompositionDefinitionEditorApp", () => {
  it("saves template recipient fields with the reusable content", async () => {
    renderEditor("template");

    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Customer reply" },
    });
    const to = screen.getByRole("combobox", { name: "To" });
    const toRow = to.parentElement?.parentElement?.parentElement;
    expect(screen.getByText("To", { selector: "label" })).toHaveClass("items-center", "w-24");
    expect(screen.getByText("To", { selector: "label" })).not.toHaveClass("border-r");
    expect(toRow).toHaveClass("border-b");
    expect(toRow?.parentElement).toHaveClass("border-t");
    expect(toRow?.parentElement).not.toHaveClass("rounded-lg", "ring-1");
    fireEvent.change(to, { target: { value: "ali" } });
    fireEvent.click(await screen.findByRole("option", { name: /Alice Local/ }));
    const cc = screen.getByRole("combobox", { name: "Cc" });
    fireEvent.change(cc, { target: { value: "team@example.com" } });
    fireEvent.blur(cc);
    fireEvent.change(screen.getByRole("textbox", { name: "Email subject" }), {
      target: { value: "Reminder" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(api.createMailTemplate).toHaveBeenCalledWith(null, expect.objectContaining({
      name: "Customer reply",
      subject: "Reminder",
      recipients: {
        to: [{ name: "Alice Local", email: "alice@example.com" }],
        cc: [{ name: null, email: "team@example.com" }],
        bcc: [],
      },
    })));
    expect(api.getLastSelectedAccount).toHaveBeenCalledOnce();
    expect(api.listContactSuggestions).toHaveBeenCalledWith("account-one", "ali", 8);
  });

  it("inserts a validated embedded image and saves it in a new signature", async () => {
    renderEditor("signature");

    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Logo signature" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Insert embedded image" }));

    await waitFor(() => expect(api.prepareCompositionDefinitionImage).toHaveBeenCalledWith(
      "logo.png",
      "image/png",
      "iVBORw==",
    ));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(api.createMailSignature).toHaveBeenCalledWith(null, {
      name: "Logo signature",
      content: expect.objectContaining({
        html: '<p><img src="data:image/png;base64,iVBORw=="></p>',
      }),
    }));
  });
});
