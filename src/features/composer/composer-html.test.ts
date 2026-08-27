import { describe, expect, it } from "vitest";

import {
  buildComposerPreviewDocument,
  ensureComposerContentHtml,
  estimateComposerDocumentHeight,
  htmlToPlainText,
  inlineImagePreviews,
  stripComposerContentEnvelope,
} from "./composer-html";

describe("composer HTML preview", () => {
  it("uses the same portable styles for editing, preview, and outgoing HTML", () => {
    const html = ensureComposerContentHtml([
      '<p style="font-size:18px">Current reply</p>',
      '<div data-nextmail-original-message=""><p>Original message</p></div>',
    ].join(""));
    const document = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
    const body = document.body.querySelector("[data-nextmail-composer-body]");
    const original = document.body.querySelector("[data-nextmail-original-message]");

    expect(html).toContain("font-size:14px");
    expect(html).toContain("line-height:1.2");
    expect(body?.innerHTML).toContain('<p style="font-size:18px">Current reply</p>');
    expect(original?.closest("[data-nextmail-composer-body]")).toBeNull();
    expect(ensureComposerContentHtml(html)).toBe(html);
    expect(buildComposerPreviewDocument(html, {}, true)).toContain("line-height:1.2");

    const fragment = stripComposerContentEnvelope(html);
    expect(fragment).toContain("Current reply");
    expect(fragment).toContain("Original message");
    expect(fragment).not.toContain("data-nextmail-composer-body");
  });

  it("resolves cached CID images and hides unavailable remote images without placeholders", () => {
    const previews = inlineImagePreviews([{
      id: "inline-one",
      fileName: "logo.png",
      contentType: "image/png",
      size: 4,
      contentId: "Logo@Example.Test",
      isInline: true,
      previewDataUrl: "data:image/png;base64,aW1hZ2U=",
    }]);
    const document = buildComposerPreviewDocument(
      '<img src="cid:logo@example.test"><img src="https://tracker.example/pixel">',
      previews,
    );
    expect(document).toContain("data:image/png;base64,aW1hZ2U=");
    expect(document).not.toContain("https://tracker.example/pixel");
    expect(document).toContain("nextmail-preview-unavailable");
  });

  it("produces plain text without leaking stylesheet source", () => {
    expect(htmlToPlainText("<style>.title{color:red}</style><p>Hello<br>World</p>"))
      .toBe("Hello\nWorld");
  });

  it("removes historical active content before rebuilding a sandbox preview", () => {
    const document = buildComposerPreviewDocument(
      '<p onclick="alert(1)">Visible</p><script>alert(2)</script><iframe srcdoc="<script>alert(3)</script>"></iframe>',
      {},
    );

    expect(document).toContain("Visible");
    expect(document).not.toContain("<script");
    expect(document).not.toContain("<iframe");
    expect(document).not.toContain("onclick");
  });

  it("expands long quoted content without imposing a maximum height", () => {
    const shortHeight = estimateComposerDocumentHeight("<p>Short message</p>", "Short message", {});
    const longText = "这是一封需要完整显示的长邮件。".repeat(180);
    const longHeight = estimateComposerDocumentHeight(
      `<table>${Array.from({ length: 30 }, (_, index) => `<tr><td>${index}</td></tr>`).join("")}</table>`,
      longText,
      {},
    );

    expect(shortHeight).toBeGreaterThanOrEqual(300);
    expect(longHeight).toBeGreaterThan(shortHeight * 3);
  });
});
