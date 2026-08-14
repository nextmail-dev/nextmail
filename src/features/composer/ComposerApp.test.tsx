import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "@/app/api";
import i18n from "@/app/i18n";
import type { ComposerBootstrap, DraftContent } from "@/app/types";
import { ComposerApp } from "./ComposerApp";

const {
  destroyMock,
  eventListenMock,
  onCloseRequestedMock,
  onDragDropEventMock,
  openMock,
  replaceSignatureMock,
  replaceTemplateMock,
  unlistenCloseMock,
  unlistenDragDropMock,
} = vi.hoisted(() => ({
  destroyMock: vi.fn(),
  eventListenMock: vi.fn(),
  onCloseRequestedMock: vi.fn(),
  onDragDropEventMock: vi.fn(),
  openMock: vi.fn(),
  replaceSignatureMock: vi.fn(() => true),
  replaceTemplateMock: vi.fn(() => true),
  unlistenCloseMock: vi.fn(),
  unlistenDragDropMock: vi.fn(),
}));

let closeHandler: ((event: { preventDefault: () => void }) => Promise<void>) | undefined;
let dragDropHandler: ((event: { payload: { type: string; paths: string[] } }) => void) | undefined;

vi.mock("@tauri-apps/api/event", () => ({ listen: eventListenMock }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: destroyMock,
    onCloseRequested: onCloseRequestedMock,
    onDragDropEvent: onDragDropEventMock,
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("@/app/api", () => ({
  api: {
    addDraftAttachments: vi.fn(),
    addDraftInlineImage: vi.fn(),
    discardDraftSession: vi.fn(),
    getComposerBootstrap: vi.fn(),
    getPreferences: vi.fn(),
    getSendJob: vi.fn(),
    listContactSuggestions: vi.fn(),
    queueDraftSend: vi.fn(),
    queueRemoteDraft: vi.fn(),
    removeDraftAttachment: vi.fn(),
    resolveContactAddresses: vi.fn(),
    renderMailSignature: vi.fn(),
    renderMailTemplate: vi.fn(),
    retrySendJob: vi.fn(),
    saveDraft: vi.fn(),
  },
  normalizeCommandError: vi.fn(() => ({
    code: "common.unexpected_error",
    params: {},
    retryable: false,
  })),
}));
vi.mock("./RichTextEditor", async () => {
  const { forwardRef, useImperativeHandle } = await import("react");
  return {
    RichTextEditor: forwardRef(function MockRichTextEditor(
      { onChange }: { onChange: (content: DraftContent) => void },
      ref,
    ) {
      useImperativeHandle(ref, () => ({
        replaceSignature: replaceSignatureMock,
        replaceTemplate: replaceTemplateMock,
      }));
      return (
        <button
          type="button"
          onClick={() => onChange({
            editorJson: "{\"type\":\"doc\"}",
            html: "<p>Changed body</p>",
            plainText: "Changed body",
          })}
        >
          Change body
        </button>
      );
    }),
  };
});

const bootstrap: ComposerBootstrap = {
  templates: [],
  signatures: [],
  sender: {
    id: "account-one",
    email: "alice@example.com",
    displayName: "Alice",
  },
  draft: {
    id: "draft-one",
    accountId: "account-one",
    status: "editing",
    recipients: { to: [], cc: [], bcc: [] },
    subject: "",
    content: {
      editorJson: "{\"type\":\"doc\"}",
      html: "",
      plainText: "",
    },
    attachments: [],
    revision: 1,
  },
};

function renderComposer() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return render(<ComposerApp accountId="account-one" draftId="draft-one" />, { wrapper: Wrapper });
}

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

beforeEach(() => {
  vi.clearAllMocks();
  closeHandler = undefined;
  dragDropHandler = undefined;
  vi.mocked(api.getPreferences).mockResolvedValue({
    theme: "system",
    accentColor: "#2563eb",
    language: "en-US",
  });
  vi.mocked(api.getComposerBootstrap).mockResolvedValue(bootstrap);
  vi.mocked(api.listContactSuggestions).mockResolvedValue([]);
  vi.mocked(api.saveDraft).mockImplementation(async (_accountId, _draftId, _recipients, _subject, content) => ({
    ...bootstrap.draft,
    content,
    revision: 2,
  }));
  vi.mocked(api.queueRemoteDraft).mockResolvedValue(undefined);
  vi.mocked(api.resolveContactAddresses).mockResolvedValue([]);
  vi.mocked(api.discardDraftSession).mockResolvedValue(undefined);
  destroyMock.mockResolvedValue(undefined);
  eventListenMock.mockResolvedValue(vi.fn());
  onCloseRequestedMock.mockImplementation((handler) => {
    closeHandler = handler;
    return Promise.resolve(unlistenCloseMock);
  });
  onDragDropEventMock.mockImplementation((handler) => {
    dragDropHandler = handler;
    return Promise.resolve(unlistenDragDropMock);
  });
});

afterEach(cleanup);

describe("ComposerApp close lifecycle", () => {
  it("keeps a recipient editable until a delimiter or blur commits it", async () => {
    renderComposer();
    const recipient = await screen.findByRole("combobox", { name: "To" });

    fireEvent.change(recipient, { target: { value: "alice@example.com" } });
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 900));
    });

    expect(screen.getByRole("combobox", { name: "To" })).toHaveValue("alice@example.com");
    expect(screen.queryByRole("button", { name: "To: alice@example.com" })).not.toBeInTheDocument();
    expect(api.saveDraft).not.toHaveBeenCalled();
  });

  it("subscribes once and saves the latest dirty draft only after close confirmation", async () => {
    renderComposer();
    const changeBody = await screen.findByRole("button", { name: "Change body" });
    await waitFor(() => expect(onCloseRequestedMock).toHaveBeenCalledOnce());

    fireEvent.click(changeBody);
    fireEvent.click(changeBody);
    fireEvent.click(changeBody);

    expect(onCloseRequestedMock).toHaveBeenCalledOnce();
    const preventDefault = vi.fn();
    await act(async () => closeHandler?.({ preventDefault }));

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(screen.getByRole("dialog")).toHaveClass("app-dialog-content");
    expect(document.querySelector(".app-dialog-overlay")).toBeInTheDocument();
    expect(api.saveDraft).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Save as draft" }));
    await waitFor(() => expect(destroyMock).toHaveBeenCalledOnce());
    expect(api.saveDraft).toHaveBeenCalledOnce();
    expect(vi.mocked(api.saveDraft).mock.calls[0]?.[4]).toEqual({
      editorJson: "{\"type\":\"doc\"}",
      html: "<p>Changed body</p>",
      plainText: "Changed body",
    });
    expect(api.queueRemoteDraft).toHaveBeenCalledWith("account-one", "draft-one");
  });

  it("discards the composing session without saving when the user declines", async () => {
    const view = renderComposer();
    await screen.findByRole("button", { name: "Change body" });
    await waitFor(() => expect(onCloseRequestedMock).toHaveBeenCalledOnce());
    const preventDefault = vi.fn();

    await act(async () => closeHandler?.({ preventDefault }));

    expect(preventDefault).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Don't save" }));
    await waitFor(() => expect(destroyMock).toHaveBeenCalledOnce());
    expect(api.saveDraft).not.toHaveBeenCalled();
    expect(api.discardDraftSession).toHaveBeenCalledWith("account-one", "draft-one");
    expect(api.queueRemoteDraft).not.toHaveBeenCalled();

    view.unmount();
    await waitFor(() => expect(unlistenCloseMock).toHaveBeenCalledOnce());
  });

  it("does not show the removed automatic draft-save status", async () => {
    renderComposer();
    await screen.findByRole("button", { name: "Change body" });

    expect(screen.queryByText("Draft saved")).not.toBeInTheDocument();
  });

  it("shows Cc by default and toggles only Bcc with a secondary button", async () => {
    renderComposer();

    const send = await screen.findByRole("button", { name: "Send" });
    expect(send.parentElement).toHaveClass("h-12", "border-b", "bg-muted/25");
    const fromLabel = await screen.findByText("From", { selector: "p" });
    const toLabel = screen.getByText("To", { selector: "label" });
    const ccLabel = screen.getByText("Cc", { selector: "label" });
    const subjectLabel = screen.getByText("Subject", { selector: "span" });
    for (const label of [fromLabel, toLabel, ccLabel, subjectLabel]) {
      expect(label).toHaveClass("text-sm", "text-justify", "[text-align-last:justify]");
      expect(label.parentElement).toHaveClass("border-b", "border-border/70");
    }
    expect(await screen.findByRole("combobox", { name: "Cc" })).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Bcc" })).not.toBeInTheDocument();
    const toggle = screen.getByRole("button", { name: "Bcc" });
    expect(toggle).toHaveClass("bg-secondary");
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(toggle);
    expect(screen.getByRole("combobox", { name: "Bcc" })).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(toggle);
    expect(screen.queryByRole("combobox", { name: "Bcc" })).not.toBeInTheDocument();
  });

  it("adds files dropped on the composer through the existing attachment command", async () => {
    vi.mocked(api.addDraftAttachments).mockResolvedValue([{
      id: "attachment-one",
      fileName: "report.pdf",
      contentType: "application/pdf",
      size: 2048,
      contentId: null,
      isInline: false,
      previewDataUrl: null,
    }]);
    renderComposer();
    await screen.findByRole("button", { name: "Change body" });
    await waitFor(() => expect(onDragDropEventMock).toHaveBeenCalledOnce());

    await act(async () => {
      dragDropHandler?.({ payload: { type: "drop", paths: ["C:\\Users\\Alice\\report.pdf"] } });
    });

    expect(api.addDraftAttachments).toHaveBeenCalledWith(
      "account-one",
      "draft-one",
      ["C:\\Users\\Alice\\report.pdf"],
    );
    expect(await screen.findByText("report.pdf")).toBeInTheDocument();
  });

  it("renders and replaces an explicitly selected template through the stable editor handle", async () => {
    vi.mocked(api.getComposerBootstrap).mockResolvedValue({
      ...bootstrap,
      templates: [{ id: "template-one", name: "Welcome", scope: "global" }],
      draft: {
        ...bootstrap.draft,
        recipients: {
          to: [{ name: null, email: "old@example.com" }],
          cc: [{ name: null, email: "old-cc@example.com" }],
          bcc: [{ name: null, email: "old-bcc@example.com" }],
        },
        subject: "Original subject",
      },
    });
    vi.mocked(api.renderMailTemplate).mockResolvedValue({
      id: "template-one",
      subject: "",
      recipients: {
        to: [{ name: "New", email: "new@example.com" }],
        cc: [],
        bcc: [{ name: null, email: "new-bcc@example.com" }],
      },
      content: {
        editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
        html: "<p>Hello</p>",
        plainText: "Hello",
      },
    });
    renderComposer();
    const template = await screen.findByRole("combobox", { name: "Template" });

    fireEvent.pointerDown(template, { button: 0, ctrlKey: false, pointerType: "mouse" });
    fireEvent.click(await screen.findByRole("option", { name: "Welcome (Global)" }));

    await waitFor(() => expect(api.renderMailTemplate).toHaveBeenCalledWith(
      "account-one",
      "template-one",
      {
        to: [{ name: null, email: "old@example.com" }],
        cc: [{ name: null, email: "old-cc@example.com" }],
        bcc: [{ name: null, email: "old-bcc@example.com" }],
      },
    ));
    await waitFor(() => expect(replaceTemplateMock).toHaveBeenCalledWith(
      "template-one",
      expect.objectContaining({ plainText: "Hello" }),
    ));
    expect(screen.getByRole("button", { name: "To: new@example.com" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "To: old@example.com" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cc: old-cc@example.com" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Bcc: new-bcc@example.com" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Bcc: old-bcc@example.com" })).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Subject" })).toHaveValue("Original subject");
  });

  it("keeps the current body when an explicitly selected template has no body content", async () => {
    vi.mocked(api.getComposerBootstrap).mockResolvedValue({
      ...bootstrap,
      templates: [{ id: "template-empty", name: "Recipients only", scope: "account" }],
    });
    vi.mocked(api.renderMailTemplate).mockResolvedValue({
      id: "template-empty",
      subject: "",
      recipients: { to: [], cc: [], bcc: [] },
      content: {
        editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
        html: "<p></p>",
        plainText: "",
      },
    });
    renderComposer();
    const template = await screen.findByRole("combobox", { name: "Template" });

    fireEvent.pointerDown(template, { button: 0, ctrlKey: false, pointerType: "mouse" });
    fireEvent.click(await screen.findByRole("option", { name: "Recipients only (Account)" }));

    await waitFor(() => expect(api.renderMailTemplate).toHaveBeenCalled());
    expect(replaceTemplateMock).not.toHaveBeenCalled();
  });
});
