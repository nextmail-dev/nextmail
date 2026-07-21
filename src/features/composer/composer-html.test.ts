import { describe, expect, it } from "vitest";

import {
  buildComposerPreviewDocument,
  estimateComposerDocumentHeight,
  htmlToPlainText,
  inlineImagePreviews,
} from "./composer-html";

describe("composer HTML preview", () => {
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
