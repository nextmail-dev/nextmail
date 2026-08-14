import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import type { DraftContent } from "@/app/types";
import {
  normalizeComposerLinkTarget,
  RichTextEditor,
  type RichTextEditorHandle,
} from "./RichTextEditor";

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
    expect(latestDocument(onChange).content?.map((node) => node.type)).toEqual([
      "paragraph",
      "paragraph",
      "nextmailSignatureDivider",
      "nextmailSignature",
      "paragraph",
    ]);

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
      expect(document.content?.some((node) => node.type === "nextmailSignatureDivider")).toBe(false);
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

    expect(screen.getByRole("toolbar")).toHaveClass("flex-wrap");
    expect(screen.getByRole("toolbar")).not.toHaveClass("overflow-x-auto");

    const editorViewport = container.querySelector(".native-scrollbar-hidden");
    expect(editorViewport).toBeInTheDocument();
    expect(editorViewport?.parentElement).toHaveAttribute("data-scrollbar-auto-hide", "true");
    expect(editorViewport).not.toHaveClass("pr-3");
    expect(container.querySelector("img[src^='https://cdn.example']")).toBeNull();
    const originalFrame = container.querySelector<HTMLIFrameElement>(".nextmail-composition-original-frame");
    expect(originalFrame).not.toBeNull();
    expect(originalFrame?.getAttribute("sandbox")).toBe("");
    expect(originalFrame?.getAttribute("scrolling")).toBe("no");
    expect(Number.parseFloat(originalFrame?.style.height ?? "0")).toBeGreaterThan(300);
    expect(originalFrame?.srcdoc).toContain('table width="600"');
    expect(originalFrame?.srcdoc).toContain("nextmail-preview-unavailable");
    const srcdocMutations: MutationRecord[] = [];
    const observer = new MutationObserver((records) => srcdocMutations.push(...records));
    observer.observe(originalFrame as HTMLIFrameElement, {
      attributes: true,
      attributeFilter: ["srcdoc"],
    });

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
        "nextmailSignatureDivider",
        "nextmailSignature",
        "paragraph",
        "nextmailOriginalMessage",
      ]);
    });
    await act(async () => { await Promise.resolve(); });
    expect(container.querySelector(".nextmail-composition-original-frame")).toBe(originalFrame);
    expect(srcdocMutations).toHaveLength(0);

    act(() => {
      expect(ref.current?.replaceSignature("signature-two", definition("Best regards"))).toBe(true);
    });
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      const document = JSON.parse(content?.editorJson ?? EMPTY) as {
        content?: Array<{ type?: string }>;
      };
      expect(document.content?.map((node) => node.type)).toEqual([
        "nextmailReply",
        "nextmailSignatureDivider",
        "nextmailSignature",
        "paragraph",
        "nextmailOriginalMessage",
      ]);
      expect(content?.editorJson).toContain("signature-two");
      expect(content?.editorJson).toContain('"sourceHtml"');
      expect(content?.html).toContain('table width="600"');
    });
    await act(async () => { await Promise.resolve(); });
    expect(container.querySelector(".nextmail-composition-original-frame")).toBe(originalFrame);
    expect(srcdocMutations).toHaveLength(0);

    act(() => {
      expect(ref.current?.replaceSignature(null)).toBe(true);
    });
    await waitFor(() => {
      const document = latestDocument(onChange);
      expect(document.content?.some((node) => node.type === "nextmailSignature")).toBe(false);
      expect(document.content?.some((node) => node.type === "nextmailSignatureDivider")).toBe(false);
      expect(document.content?.some((node) => node.type === "nextmailOriginalMessage")).toBe(true);
    });
    observer.disconnect();

    await selectMoreFormatting("HTML source");
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

  it("inserts a selected image through the same cached CID path", async () => {
    const onChange = vi.fn<(content: DraftContent) => void>();
    const onAddInlineImage = vi.fn(async () => ({
      id: "inline-selected",
      fileName: "selected.png",
      contentType: "image/png",
      size: 12,
      contentId: "inline-selected@nextmail.local",
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
    await openMoreFormatting();
    expect(await screen.findByRole("menuitem", { name: "Insert image" })).toBeInTheDocument();
    const input = container.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input).not.toBeNull();
    const image = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], "selected.png", {
      type: "image/png",
    });
    fireEvent.change(input as HTMLInputElement, { target: { files: [image] } });

    await waitFor(() => expect(onAddInlineImage).toHaveBeenCalledWith(image));
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toContain('src="cid:inline-selected@nextmail.local"');
      expect(content?.editorJson).not.toContain("data:image/png");
    });
  });

  it("inserts four preserved spaces instead of moving focus on Tab", async () => {
    const onChange = vi.fn<(content: DraftContent) => void>();
    const { container } = render(<RichTextEditor initialJson={EMPTY} onChange={onChange} />);
    const editable = await waitFor(() => {
      const value = container.querySelector<HTMLElement>(".ProseMirror");
      expect(value).not.toBeNull();
      return value as HTMLElement;
    });
    editable.focus();

    expect(fireEvent.keyDown(editable, { key: "Tab" })).toBe(false);

    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toContain("&nbsp;&nbsp;&nbsp;&nbsp;");
    });
    expect(editable.textContent).toContain("\u00a0".repeat(4));
    expect(editable).toHaveFocus();
  });

  it("inserts a prepared reusable-definition image as a persistent data source", async () => {
    const onChange = vi.fn<(content: DraftContent) => void>();
    const onAddInlineImage = vi.fn(async () => ({
      fileName: "signature-logo.png",
      contentType: "image/png",
      size: 12,
      contentId: null,
      previewDataUrl: "data:image/png;base64,aW1hZ2U=",
    }));
    const { container } = render(
      <RichTextEditor
        initialJson={EMPTY}
        onChange={onChange}
        onAddInlineImage={onAddInlineImage}
      />,
    );
    const input = await waitFor(() => {
      const value = container.querySelector<HTMLInputElement>('input[type="file"]');
      expect(value).not.toBeNull();
      return value as HTMLInputElement;
    });
    const image = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], "signature-logo.png", {
      type: "image/png",
    });
    fireEvent.change(input, { target: { files: [image] } });

    await waitFor(() => expect(onAddInlineImage).toHaveBeenCalledWith(image));
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toContain('src="data:image/png;base64,aW1hZ2U="');
      expect(content?.editorJson).toContain("data:image/png;base64,aW1hZ2U=");
      expect(content?.editorJson).not.toContain("previewSrc");
    });
  });

  it("imports sanitized clipboard HTML with scoped styles and editable structure", async () => {
    const onChange = vi.fn<(content: DraftContent) => void>();
    const onAddInlineImage = vi.fn(async () => ({
      id: "inline-rich-paste",
      fileName: "copied.png",
      contentType: "image/png",
      size: 12,
      contentId: "inline-rich-paste@nextmail.local",
      isInline: true,
      previewDataUrl: "data:image/png;base64,aW1hZ2U=",
    }));
    const onSanitizeHtml = vi.fn(async () => [
      '<div data-nextmail-pasted-html="">',
      '<style data-nextmail-compose-style="">[data-nextmail-pasted-html] .copied{color:#123456}</style>',
      '<p class="copied" id="copied-line" style="font-size:18px">',
      '<span style="font-family:Arial;color:#654321">Styled paste</span><img src="https://example.test/copied.png">',
      "</p></div>",
    ].join(""));
    const { container } = render(
      <RichTextEditor
        initialJson={EMPTY}
        onChange={onChange}
        onAddInlineImage={onAddInlineImage}
        onSanitizeHtml={onSanitizeHtml}
      />,
    );
    const editable = await waitFor(() => container.querySelector<HTMLElement>(".ProseMirror") as HTMLElement);
    const image = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], "copied.png", {
      type: "image/png",
    });
    fireEvent.paste(editable, {
      clipboardData: {
        files: [image],
        items: [],
        getData: vi.fn((type: string) => type === "text/html"
          ? '<p class="copied" style="font-size:18px">Styled paste</p>'
          : "Styled paste"),
      },
    });

    await waitFor(() => expect(onSanitizeHtml).toHaveBeenCalledOnce());
    await waitFor(() => expect(onAddInlineImage).toHaveBeenCalledWith(image));
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toContain("data-nextmail-pasted-html");
      expect(content?.html).toContain("[data-nextmail-pasted-html] .copied");
      expect(content?.html).toContain('class="copied"');
      expect(content?.html).toContain('id="copied-line"');
      expect(content?.html).toContain("font-family: Arial");
      expect(content?.html).toContain('src="cid:inline-rich-paste@nextmail.local"');
      expect(content?.html).not.toContain("example.test/copied.png");
      expect(content?.editorJson).not.toContain("data:image/png");
      expect(content?.plainText).toContain("Styled paste");
    });
  });

  it("keeps the persisted HTML source exact until the rich editor is actually changed", async () => {
    const exactHtml = 'Bare text <span style="font-size:19px">without a paragraph wrapper</span>';
    render(
      <RichTextEditor
        initialJson={EMPTY}
        initialHtml={exactHtml}
        onChange={vi.fn()}
      />,
    );

    await selectMoreFormatting("HTML source");
    const source = await screen.findByRole("textbox", { name: "HTML source" });
    expect(source.textContent).toBe(exactHtml);

    await selectMoreFormatting("HTML source");
    await selectMoreFormatting("HTML source");
    expect(await screen.findByRole("textbox", { name: "HTML source" })).toHaveTextContent(exactHtml);
  });

  it("preserves inline div structure and image dimensions during rich serialization", async () => {
    const ref = createRef<RichTextEditorHandle>();
    const onChange = vi.fn<(content: DraftContent) => void>();
    const initialJson = JSON.stringify({
      type: "doc",
      content: [{
        type: "emailInlineBlock",
        attrs: {
          emailClass: "email-line",
          emailId: "line-one",
          emailStyle: "text-align: center",
        },
        content: [
          { type: "text", text: "Inline div" },
          {
            type: "nextmailImage",
            attrs: {
              src: "data:image/png;base64,aW1hZ2U=",
              alt: "Sized",
              width: "72",
              height: "36",
              emailStyle: "vertical-align: bottom; width: 72px; height: 36px",
              emailClass: "mail-logo",
              emailId: "logo-one",
              align: "bottom",
              border: "0",
              hspace: "4",
              vspace: "2",
            },
          },
        ],
      }],
    });
    const { container } = render(
      <RichTextEditor ref={ref} initialJson={initialJson} onChange={onChange} />,
    );
    await waitFor(() => expect(ref.current).not.toBeNull());

    const image = container.querySelector<HTMLImageElement>(".nextmail-email-image");
    expect(image).toHaveAttribute("width", "72");
    expect(image).toHaveAttribute("height", "36");
    expect(image).toHaveAttribute("hspace", "4");
    expect(image).toHaveStyle({ width: "72px", height: "36px", verticalAlign: "bottom" });

    act(() => {
      expect(ref.current?.replaceSignature("signature-one", definition("Regards"))).toBe(true);
    });
    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toMatch(
        /<div style="text-align: center;?" class="email-line" id="line-one">Inline div/,
      );
      expect(content?.html).not.toContain("<div><p>Inline div");
      expect(content?.html).toContain('width="72"');
      expect(content?.html).toContain('height="36"');
      expect(content?.html).toContain('class="mail-logo"');
    });
  });

  it("inserts a validated link at an empty selection", async () => {
    const onChange = vi.fn<(content: DraftContent) => void>();
    render(<RichTextEditor initialJson={EMPTY} onChange={onChange} />);

    await selectMoreFormatting("Insert or edit link");
    fireEvent.change(screen.getByRole("textbox", { name: "Link address" }), {
      target: { value: " HTTPS://Example.COM:443/news " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Apply link" }));

    await waitFor(() => {
      const content = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0];
      expect(content?.html).toContain(
        '<a target="_blank" rel="noopener noreferrer" href="https://example.com/news">https://example.com/news</a>',
      );
    });
  });
});

describe("normalizeComposerLinkTarget", () => {
  it("normalizes supported links and rejects active, credentialed, or confusing targets", () => {
    expect(normalizeComposerLinkTarget(" HTTPS://Example.COM:443/news ")).toBe(
      "https://example.com/news",
    );
    expect(normalizeComposerLinkTarget("//example.com/news")).toBe("https://example.com/news");
    expect(normalizeComposerLinkTarget("mailto:reader@example.com?subject=Hello")).toBe(
      "mailto:reader@example.com?subject=Hello",
    );

    for (const target of [
      "javascript:alert(1)",
      "data:text/html,hello",
      "file:///C:/secret.txt",
      "https://user:secret@example.com/",
      "https://example.com\\@attacker.invalid/",
      "https://example.com/%0d%0aHeader:value",
      "https://example.com/\u202emoc.live",
      "mailto:",
    ]) {
      expect(normalizeComposerLinkTarget(target), target).toBeNull();
    }
  });
});

async function openMoreFormatting() {
  fireEvent.pointerDown(await screen.findByRole("button", { name: "More formatting options" }), {
    button: 0,
    ctrlKey: false,
  });
}

async function selectMoreFormatting(name: string) {
  await openMoreFormatting();
  fireEvent.click(await screen.findByRole("menuitem", { name }));
}

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
