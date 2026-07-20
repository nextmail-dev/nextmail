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
  openMock,
  replaceSignatureMock,
  replaceTemplateMock,
  unlistenCloseMock,
} = vi.hoisted(() => ({
  destroyMock: vi.fn(),
  eventListenMock: vi.fn(),
  onCloseRequestedMock: vi.fn(),
  openMock: vi.fn(),
  replaceSignatureMock: vi.fn(() => true),
  replaceTemplateMock: vi.fn(() => true),
  unlistenCloseMock: vi.fn(),
}));

let closeHandler: ((event: { preventDefault: () => void }) => Promise<void>) | undefined;

vi.mock("@tauri-apps/api/event", () => ({ listen: eventListenMock }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    destroy: destroyMock,
    onCloseRequested: onCloseRequestedMock,
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("@/app/api", () => ({
  api: {
    addDraftAttachments: vi.fn(),
    discardEmptyDraft: vi.fn(),
    getComposerBootstrap: vi.fn(),
    getPreferences: vi.fn(),
    getSendJob: vi.fn(),
    queueDraftSend: vi.fn(),
    queueRemoteDraft: vi.fn(),
    removeDraftAttachment: vi.fn(),
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
  vi.mocked(api.getPreferences).mockResolvedValue({
    theme: "system",
    accentColor: "#2563eb",
    language: "en-US",
  });
  vi.mocked(api.getComposerBootstrap).mockResolvedValue(bootstrap);
  vi.mocked(api.saveDraft).mockImplementation(async (_accountId, _draftId, _recipients, _subject, content) => ({
    ...bootstrap.draft,
    content,
    revision: 2,
  }));
  vi.mocked(api.queueRemoteDraft).mockResolvedValue(undefined);
  vi.mocked(api.discardEmptyDraft).mockResolvedValue(undefined);
  destroyMock.mockResolvedValue(undefined);
  eventListenMock.mockResolvedValue(vi.fn());
  onCloseRequestedMock.mockImplementation((handler) => {
    closeHandler = handler;
    return Promise.resolve(unlistenCloseMock);
  });
});

afterEach(cleanup);

describe("ComposerApp close lifecycle", () => {
  it("subscribes once across body changes and saves the latest dirty draft on close", async () => {
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
    expect(api.saveDraft).toHaveBeenCalledOnce();
    expect(vi.mocked(api.saveDraft).mock.calls[0]?.[4]).toEqual({
      editorJson: "{\"type\":\"doc\"}",
      html: "<p>Changed body</p>",
      plainText: "Changed body",
    });
    expect(api.queueRemoteDraft).toHaveBeenCalledWith("account-one", "draft-one");
    expect(api.discardEmptyDraft).toHaveBeenCalledWith("account-one", "draft-one");
    expect(destroyMock).toHaveBeenCalledOnce();
  });

  it("keeps the existing empty-draft close path without an unnecessary save", async () => {
    const view = renderComposer();
    await screen.findByRole("button", { name: "Change body" });
    await waitFor(() => expect(onCloseRequestedMock).toHaveBeenCalledOnce());
    const preventDefault = vi.fn();

    await act(async () => closeHandler?.({ preventDefault }));

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(api.saveDraft).not.toHaveBeenCalled();
    expect(api.queueRemoteDraft).toHaveBeenCalledWith("account-one", "draft-one");
    expect(api.discardEmptyDraft).toHaveBeenCalledWith("account-one", "draft-one");
    expect(destroyMock).toHaveBeenCalledOnce();

    view.unmount();
    await waitFor(() => expect(unlistenCloseMock).toHaveBeenCalledOnce());
  });

  it("renders and replaces an explicitly selected template through the stable editor handle", async () => {
    vi.mocked(api.getComposerBootstrap).mockResolvedValue({
      ...bootstrap,
      templates: [{ id: "template-one", name: "Welcome", scope: "global" }],
    });
    vi.mocked(api.renderMailTemplate).mockResolvedValue({
      id: "template-one",
      subject: "Hello",
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
      { to: [], cc: [], bcc: [] },
    ));
    expect(replaceTemplateMock).toHaveBeenCalledWith("template-one", expect.objectContaining({
      plainText: "Hello",
    }));
  });
});
