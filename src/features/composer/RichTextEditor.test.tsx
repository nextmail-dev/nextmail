import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import type { DraftContent } from "@/app/types";
import { RichTextEditor, type RichTextEditorHandle } from "./RichTextEditor";

const EMPTY = '{"type":"doc","content":[{"type":"paragraph"}]}';

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(cleanup);

describe("RichTextEditor composition nodes", () => {
  it("replaces one stable signature node and removes it without touching the body", async () => {
    const ref = createRef<RichTextEditorHandle>();
    const onChange = vi.fn<(content: DraftContent) => void>();
    render(<RichTextEditor ref={ref} initialJson={EMPTY} onChange={onChange} />);
    await waitFor(() => expect(ref.current).not.toBeNull());

    act(() => {
      expect(ref.current?.replaceSignature("signature-one", definition("First"))).toBe(true);
    });
    await waitFor(() => expect(onChange).toHaveBeenCalled());

    act(() => {
      expect(ref.current?.replaceSignature("signature-two", definition("Second"))).toBe(true);
    });
    await waitFor(() => {
      const document = latestDocument(onChange);
      expect(document.content?.filter((node) => node.type === "nextmailSignature")).toHaveLength(1);
      expect(JSON.stringify(document)).toContain("signature-two");
      expect(JSON.stringify(document)).toContain("Second");
      expect(JSON.stringify(document)).not.toContain("signature-one");
    });

    act(() => {
      expect(ref.current?.replaceSignature(null)).toBe(true);
    });
    await waitFor(() => {
      const document = latestDocument(onChange);
      expect(document.content?.some((node) => node.type === "nextmailSignature")).toBe(false);
    });
  });

  it("keeps original HTML authoritative in a sandbox instead of normalizing its table", async () => {
    const ref = createRef<RichTextEditorHandle>();
    const onChange = vi.fn<(content: DraftContent) => void>();
    const sourceHtml = [
      '<style data-nextmail-compose-style="">[data-nextmail-original-message] .mail-title{font-size:18px}</style>',
      '<style data-nextmail-compose-style="">body{position:fixed;z-index:9999}</style>',
      '<p style="color:#123456">Sender wrote:</p>',
      '<table width="600" cellpadding="0" cellspacing="0"><tbody><tr>',
      '<td width="420" style="background-color:#ffffff"><strong>Original</strong></td>',
      "</tr></tbody></table>",
      '<p><a href="https://example.com/account">Account</a>',
      '<img src="https://cdn.example/banner.png" alt="Banner"></p>',
    ].join("");
    const initialJson = JSON.stringify({
      type: "doc",
      content: [
        { type: "nextmailReply", content: [{ type: "paragraph" }] },
        { type: "paragraph" },
        {
          type: "nextmailOriginalMessage",
          attrs: { sourceHtml },
          content: [{ type: "paragraph", content: [{ type: "text", text: "Fallback" }] }],
        },
      ],
    });
    const { container } = render(
      <RichTextEditor ref={ref} initialJson={initialJson} onChange={onChange} />,
    );
    await waitFor(() => expect(ref.current).not.toBeNull());

    expect(container.querySelector(".nextmail-editor-scroll")).toHaveClass("overflow-y-scroll");
    expect(container.querySelector("img[src^='https://cdn.example']")).toBeNull();
    const originalFrame = container.querySelector<HTMLIFrameElement>(".nextmail-composition-original-frame");
    expect(originalFrame).not.toBeNull();
    expect(originalFrame?.getAttribute("sandbox")).toBe("");
    expect(originalFrame?.getAttribute("scrolling")).toBe("no");
    expect(Number.parseFloat(originalFrame?.style.height ?? "0")).toBeGreaterThan(300);
    expect(originalFrame?.srcdoc).toContain('table width="600"');
    expect(originalFrame?.srcdoc).toContain("nextmail-preview-unavailable");

    act(() => {
      expect(ref.current?.replaceSignature("signature-one", definition("Regards"))).toBe(true);
    });
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.editorJson).not.toContain('"type":"table"');
      expect(content?.editorJson).toContain('"sourceHtml"');
      expect(content?.html).toContain('table width="600"');
      expect(content?.html).toContain("[data-nextmail-original-message] .mail-title");
      expect(content?.html).toContain('src="https://cdn.example/banner.png"');
      expect(content?.html).toContain('href="https://example.com/account"');

      const document = JSON.parse(content?.editorJson ?? EMPTY) as {
        content?: Array<{ type?: string }>;
      };
      expect(document.content?.map((node) => node.type)).toEqual([
        "nextmailReply",
        "paragraph",
        "nextmailSignature",
        "nextmailOriginalMessage",
      ]);
    });

    fireEvent.click(screen.getByRole("button", { name: "HTML source" }));
    expect(screen.getByRole("textbox", { name: "HTML source" })).toBeInTheDocument();
    expect(screen.getByTitle("HTML preview")).toHaveAttribute("sandbox", "");
  });

  it("inserts a pasted cached image as CID HTML without persisting its data URL", async () => {
    const onChange = vi.fn<(content: DraftContent) => void>();
    const onAddInlineImage = vi.fn(async () => ({
      id: "inline-one",
      fileName: "pasted.png",
      contentType: "image/png",
      size: 12,
      contentId: "inline-one@nextmail.local",
      isInline: true,
      previewDataUrl: "data:image/png;base64,aW1hZ2U=",
    }));
    const { container } = render(
      <RichTextEditor
        initialJson={EMPTY}
        onChange={onChange}
        onAddInlineImage={onAddInlineImage}
      />,
    );
    const editable = await waitFor(() => {
      const value = container.querySelector<HTMLElement>(".ProseMirror");
      expect(value).not.toBeNull();
      return value as HTMLElement;
    });
    const image = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], "pasted.png", {
      type: "image/png",
    });
    fireEvent.paste(editable, {
      clipboardData: {
        files: [image],
        items: [],
        getData: vi.fn(() => ""),
      },
    });

    await waitFor(() => expect(onAddInlineImage).toHaveBeenCalledWith(image));
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toContain('src="cid:inline-one@nextmail.local"');
      expect(content?.editorJson).toContain("inline-one@nextmail.local");
      expect(content?.editorJson).not.toContain("data:image/png");
      expect(container.querySelector<HTMLImageElement>(".nextmail-email-image")?.src)
        .toBe("data:image/png;base64,aW1hZ2U=");
    });
  });
});

function definition(text: string): DraftContent {
  return {
    editorJson: JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text }] }],
    }),
    html: `<p>${text}</p>`,
    plainText: text,
  };
}

function latestDocument(onChange: ReturnType<typeof vi.fn<(content: DraftContent) => void>>) {
  const call = onChange.mock.calls[onChange.mock.calls.length - 1];
  return JSON.parse(call?.[0].editorJson ?? EMPTY) as {
    content?: Array<{ type?: string }>;
  };
}
