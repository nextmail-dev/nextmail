import type { DraftAttachmentSummary } from "@/app/types";

export function inlineImagePreviews(attachments: DraftAttachmentSummary[]) {
  return Object.fromEntries(attachments.flatMap((attachment) => (
    attachment.isInline && attachment.contentId && attachment.previewDataUrl
      ? [[normalizeContentId(attachment.contentId), attachment.previewDataUrl]]
      : []
  )));
}

export function normalizeContentId(value: string) {
  return value.trim().replace(/^<|>$/g, "").toLocaleLowerCase();
}

export function buildComposerPreviewDocument(
  html: string,
  previews: Record<string, string>,
) {
  const document = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  for (const image of document.body.querySelectorAll("img")) {
    image.removeAttribute("srcset");
    const source = image.getAttribute("src")?.trim() ?? "";
    if (source.toLocaleLowerCase().startsWith("cid:")) {
      const preview = previews[normalizeContentId(source.slice(4))];
      if (preview) image.setAttribute("src", preview);
      else hideUnavailableImage(image);
    } else if (!source.toLocaleLowerCase().startsWith("data:image/")) {
      hideUnavailableImage(image);
    }
  }
  return [
    "<!doctype html><html><head><meta charset=\"utf-8\">",
    "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\">",
    "<style>html{color-scheme:light}body{margin:0;min-width:0}.nextmail-preview-unavailable{display:none!important}</style>",
    `</head><body>${document.body.innerHTML}</body></html>`,
  ].join("");
}

export function estimateComposerDocumentHeight(
  html: string,
  plainText: string,
  previews: Record<string, string>,
) {
  const document = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  document.body.querySelectorAll("style,script,noscript").forEach((element) => element.remove());
  const text = plainText.trim() || document.body.textContent?.trim() || "";
  const weightedCharacters = Array.from(text).reduce(
    (total, character) => total + (/[^\u0000-\u00ff]/.test(character) ? 1 : 0.55),
    0,
  );
  const explicitBreaks = document.body.querySelectorAll("br").length;
  const textLines = Math.ceil(weightedCharacters / 34) + explicitBreaks;
  const structuralLines = Math.ceil(
    document.body.querySelectorAll("p,li,h1,h2,h3,h4,h5,h6,blockquote").length * 0.8,
  );
  const tableLines = document.body.querySelectorAll("tr").length * 1.8;
  let imageHeight = 0;
  for (const image of document.body.querySelectorAll("img")) {
    const source = image.getAttribute("src")?.trim() ?? "";
    const available = source.toLocaleLowerCase().startsWith("data:image/")
      || (source.toLocaleLowerCase().startsWith("cid:")
        && Boolean(previews[normalizeContentId(source.slice(4))]));
    if (!available) continue;
    const declaredHeight = numericDimension(image.getAttribute("height"))
      ?? numericStyleDimension(image.getAttribute("style"), "height");
    const declaredWidth = numericDimension(image.getAttribute("width"))
      ?? numericStyleDimension(image.getAttribute("style"), "width");
    imageHeight += declaredHeight ?? (declaredWidth && declaredWidth <= 96 ? declaredWidth : 220);
  }
  const contentLines = Math.max(textLines, structuralLines, tableLines, 8);
  return Math.ceil((contentLines * 25 + imageHeight + 96) * 1.3);
}

function numericDimension(value: string | null) {
  if (!value) return null;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) && parsed > 0 ? Math.min(parsed, 4_000) : null;
}

function numericStyleDimension(style: string | null, property: string) {
  if (!style) return null;
  const match = style.match(new RegExp(`(?:^|;)\\s*${property}\\s*:\\s*([0-9.]+)px`, "i"));
  return numericDimension(match?.[1] ?? null);
}

function hideUnavailableImage(image: HTMLImageElement) {
  image.removeAttribute("src");
  image.classList.add("nextmail-preview-unavailable");
  image.setAttribute("aria-hidden", "true");
}

export function htmlToPlainText(html: string) {
  const document = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  document.body.querySelectorAll("style,script,noscript").forEach((element) => element.remove());
  for (const lineBreak of document.body.querySelectorAll("br")) {
    lineBreak.replaceWith(document.createTextNode("\n"));
  }
  for (const block of document.body.querySelectorAll("p,div,h1,h2,h3,h4,h5,h6,li,blockquote,tr")) {
    block.append(document.createTextNode("\n"));
  }
  return (document.body.textContent ?? "")
    .replace(/\u00a0/g, " ")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
