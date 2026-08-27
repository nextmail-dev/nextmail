import type { DraftAttachmentSummary } from "@/app/types";

export const COMPOSER_CONTENT_STYLE = [
  '[data-nextmail-composer-body]{color:#202124;background:#fff;font-family:system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI","Microsoft YaHei",Arial,sans-serif;font-size:14px;line-height:1.2;overflow-wrap:anywhere}',
  '[data-nextmail-composer-body] p{margin:0 0 .8em}',
  '[data-nextmail-composer-body] h1,[data-nextmail-composer-body] h2,[data-nextmail-composer-body] h3{font-size:inherit;font-weight:inherit;margin:1.1em 0 .55em;line-height:1.25}',
  '[data-nextmail-composer-body] blockquote{margin:1em 0;padding-left:1rem;border-left:3px solid #8a94a6;color:#5f6d80}',
  '[data-nextmail-composer-body] ul,[data-nextmail-composer-body] ol{margin:.8em 0;padding-left:1.5rem;list-style-type:none}',
  '[data-nextmail-composer-body] a{color:inherit;text-decoration:inherit}',
  '[data-nextmail-composer-body] span[style*="background-color"]{color:#202124}',
  '[data-nextmail-composer-body] .nextmail-composition-template,[data-nextmail-composer-body] .nextmail-composition-signature{margin:0;padding:0;border:0;border-radius:0;background:transparent}',
  '[data-nextmail-composer-body] .nextmail-composition-signature>:last-child{margin-bottom:0}',
  '[data-nextmail-original-message]{margin-top:.9rem;overflow-wrap:normal}',
  '[data-nextmail-composer-body] table{table-layout:auto}',
  '[data-nextmail-composer-body] td,[data-nextmail-composer-body] th{vertical-align:top}',
  '[data-nextmail-composer-body] img{display:inline-block}',
].join("");

export function ensureComposerContentHtml(html: string) {
  const document = fragmentDocument(html);
  const directChildren = Array.from(document.body.children);
  const composerBodies = directChildren.filter((element) => (
    element.hasAttribute("data-nextmail-composer-body")
  ));
  const composerStyles = directChildren.filter((element): element is HTMLStyleElement => (
    element instanceof HTMLStyleElement && isComposerContentStyle(element.textContent ?? "")
  ));
  const hasUnwrappedContent = Array.from(document.body.childNodes).some((node) => {
    if (node instanceof HTMLStyleElement) return false;
    if (node instanceof HTMLElement) {
      return !node.hasAttribute("data-nextmail-composer-body")
        && !node.hasAttribute("data-nextmail-original-message");
    }
    return Boolean(node.textContent?.trim());
  });
  if (composerBodies.length && composerStyles.length && !hasUnwrappedContent) return html;

  for (const style of composerStyles) style.remove();
  for (const body of composerBodies) body.replaceWith(...Array.from(body.childNodes));

  const groups: Node[][] = [];
  let current: Node[] = [];
  const flush = () => {
    if (current.length) groups.push(current);
    current = [];
  };
  for (const node of Array.from(document.body.childNodes)) {
    if (
      node instanceof HTMLStyleElement
      || (node instanceof HTMLElement && node.hasAttribute("data-nextmail-original-message"))
    ) {
      flush();
    } else {
      current.push(node);
    }
  }
  flush();

  for (const group of groups) {
    const body = document.createElement("div");
    body.setAttribute("data-nextmail-composer-body", "");
    if (group[0]?.parentNode) group[0].parentNode.insertBefore(body, group[0]);
    body.append(...group);
  }
  if (!document.body.querySelector(":scope > [data-nextmail-composer-body]")) {
    const body = document.createElement("div");
    body.setAttribute("data-nextmail-composer-body", "");
    document.body.prepend(body);
  }

  const style = document.createElement("style");
  style.setAttribute("data-nextmail-composer-style", "");
  style.textContent = COMPOSER_CONTENT_STYLE;
  document.body.prepend(style);
  return document.body.innerHTML;
}

export function stripComposerContentEnvelope(html: string) {
  const document = fragmentDocument(html);
  for (const style of Array.from(document.body.children)) {
    if (style instanceof HTMLStyleElement && isComposerContentStyle(style.textContent ?? "")) {
      style.remove();
    }
  }
  for (const body of Array.from(document.body.children)) {
    if (body.hasAttribute("data-nextmail-composer-body")) {
      body.replaceWith(...Array.from(body.childNodes));
    }
  }
  return document.body.innerHTML;
}

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
  applyComposerDefaults = false,
) {
  if (applyComposerDefaults) html = ensureComposerContentHtml(html);
  const document = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  // Rust remains the authoritative sanitizer. This second, deliberately
  // narrow guard prevents historical editor content from handing active
  // elements back to an about:srcdoc preview when a composition node changes.
  // An empty sandbox would block the script, but WebView would still report a
  // noisy attempted execution for every signature/template replacement.
  document.body
    .querySelectorAll("base,embed,form,iframe,link,math,meta,noscript,object,script,svg")
    .forEach((element) => element.remove());
  for (const element of document.body.querySelectorAll("*")) {
    for (const attribute of Array.from(element.attributes)) {
      if (attribute.name.toLocaleLowerCase().startsWith("on")) {
        element.removeAttribute(attribute.name);
      }
    }
  }
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

function fragmentDocument(html: string) {
  return new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
}

function isComposerContentStyle(css: string) {
  const compact = css.replace(/\s+/g, "").toLocaleLowerCase();
  return compact.includes("[data-nextmail-composer-body]{")
    && compact.includes("font-size:14px")
    && compact.includes("line-height:1.2");
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
