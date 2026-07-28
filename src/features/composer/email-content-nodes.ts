import { Extension, Mark, mergeAttributes, Node } from "@tiptap/core";
import {
  Table,
  TableCell,
  TableHeader,
  TableRow,
} from "@tiptap/extension-table";

const ORIGINAL_SCOPE = "[data-nextmail-original-message]";
const PASTED_SCOPE = "[data-nextmail-pasted-html]";
const GENERIC_EMAIL_DIV =
  "div:not([data-nextmail-reply]):not([data-nextmail-original-message]):not([data-nextmail-template-id]):not([data-nextmail-signature-id])";
const BLOCK_CONTENT_TAGS = new Set([
  "address",
  "article",
  "aside",
  "blockquote",
  "details",
  "dialog",
  "div",
  "dl",
  "fieldset",
  "figure",
  "footer",
  "form",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hr",
  "main",
  "nav",
  "ol",
  "p",
  "pre",
  "section",
  "style",
  "table",
  "ul",
]);

function safeComposerStylesheet(value: unknown) {
  if (typeof value !== "string" || value.length > 256 * 1024) return "";
  if (/(?:\/\*|url\s*\(|@import|@font-face|expression\s*\(|position\s*:\s*fixed|z-index\s*:|<\/style)/i.test(value)) {
    return "";
  }
  const withoutMedia = value.replace(/@media\s+[^{}]+\{/gi, "");
  if (withoutMedia.includes("@")) return "";
  const headers = Array.from(withoutMedia.matchAll(/(?:^|})\s*([^{}]+)\{/g));
  if (!headers.length || headers.some((match) => match[1]
    .split(",")
    .some((selector) => {
      const value = selector.trim();
      const scope = [ORIGINAL_SCOPE, PASTED_SCOPE]
        .find((candidate) => value === candidate || value.startsWith(`${candidate} `));
      if (!scope) return true;
      if (value === scope) return false;
      const relative = value.slice(scope.length).trimStart();
      return ["+", "~"].some((prefix) => relative.startsWith(prefix));
    }))) {
    return "";
  }
  return value;
}

function preservedAttribute(
  htmlName: string,
  attributeName = htmlName === "style" ? "emailStyle" : htmlName,
) {
  return {
    default: null,
    parseHTML: (element: HTMLElement) => element.getAttribute(htmlName),
    renderHTML: (attributes: Record<string, unknown>) => {
      const value = attributes[attributeName];
      return typeof value === "string" ? { [htmlName]: value } : {};
    },
  };
}

export const EmailFormattingAttributes = Extension.create({
  name: "emailFormattingAttributes",
  addGlobalAttributes() {
    return [{
      types: [
        "paragraph",
        "heading",
        "blockquote",
        "bulletList",
        "orderedList",
        "listItem",
        "emailInlineBlock",
        "emailBlock",
        "table",
        "tableRow",
        "tableCell",
        "tableHeader",
        "link",
        "bold",
        "italic",
        "strike",
        "underline",
      ],
      attributes: {
        emailStyle: preservedAttribute("style"),
        emailClass: preservedAttribute("class", "emailClass"),
        emailId: preservedAttribute("id", "emailId"),
        align: preservedAttribute("align"),
        dir: preservedAttribute("dir"),
      },
    }];
  },
});

export const EmailInlineBlock = Node.create({
  name: "emailInlineBlock",
  group: "block",
  content: "inline*",
  parseHTML() {
    return [{
      tag: GENERIC_EMAIL_DIV,
      priority: 110,
      getAttrs: (element) => element instanceof HTMLElement
        && !Array.from(element.children)
          .some((child) => BLOCK_CONTENT_TAGS.has(child.tagName.toLocaleLowerCase()))
        ? {}
        : false,
    }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", HTMLAttributes, 0];
  },
});

export const EmailBlock = Node.create({
  name: "emailBlock",
  group: "block",
  content: "block*",
  addAttributes() {
    return {
      pastedHtml: {
        default: false,
        parseHTML: (element) => element.hasAttribute("data-nextmail-pasted-html"),
        renderHTML: (attributes) => attributes.pastedHtml
          ? { "data-nextmail-pasted-html": "" }
          : {},
      },
    };
  },
  parseHTML() {
    return [{
      tag: GENERIC_EMAIL_DIV,
    }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", HTMLAttributes, 0];
  },
});

export const EmailSpan = Mark.create({
  name: "emailSpan",
  addAttributes() {
    return {
      emailStyle: preservedAttribute("style"),
      emailClass: preservedAttribute("class", "emailClass"),
      emailId: preservedAttribute("id", "emailId"),
      dir: preservedAttribute("dir"),
    };
  },
  parseHTML() {
    return [{ tag: "span[style]" }, { tag: "span[class]" }, { tag: "span[id]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["span", HTMLAttributes, 0];
  },
});

export const EmailFont = Mark.create({
  name: "emailFont",
  addAttributes() {
    return {
      color: preservedAttribute("color"),
      face: preservedAttribute("face"),
      size: preservedAttribute("size"),
      emailStyle: preservedAttribute("style"),
    };
  },
  parseHTML() {
    return [{ tag: "font" }];
  },
  renderHTML({ HTMLAttributes }) {
    const { emailStyle, ...attributes } = HTMLAttributes;
    return ["font", mergeAttributes(
      attributes,
      typeof emailStyle === "string" ? { style: emailStyle } : {},
    ), 0];
  },
});

export const EmailStylesheet = Node.create({
  name: "emailStylesheet",
  group: "block",
  atom: true,
  selectable: false,
  addAttributes() {
    return {
      css: {
        default: "",
        rendered: false,
        parseHTML: (element) => safeComposerStylesheet(element.textContent),
      },
    };
  },
  parseHTML() {
    return [{ tag: "style[data-nextmail-compose-style]" }];
  },
  renderHTML({ node }) {
    return ["style", { "data-nextmail-compose-style": "" }, safeComposerStylesheet(node.attrs.css)];
  },
  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement("style");
      dom.dataset.nextmailComposeStyle = "";
      dom.textContent = safeComposerStylesheet(node.attrs.css);
      return { dom };
    };
  },
});

export const NextMailImage = Node.create({
  name: "nextmailImage",
  group: "inline",
  inline: true,
  atom: true,
  draggable: true,
  addAttributes() {
    return {
      src: { default: null },
      contentId: {
        default: null,
        rendered: false,
        parseHTML: (element) => {
          const source = element.getAttribute("src")?.trim() ?? "";
          return source.toLocaleLowerCase().startsWith("cid:") ? source.slice(4) : null;
        },
      },
      previewSrc: {
        default: null,
        rendered: false,
        parseHTML: (element) => element.getAttribute("data-nextmail-preview-src"),
      },
      alt: { default: null },
      title: { default: null },
      width: { default: null },
      height: { default: null },
      emailStyle: preservedAttribute("style"),
      emailClass: preservedAttribute("class", "emailClass"),
      emailId: preservedAttribute("id", "emailId"),
      align: preservedAttribute("align"),
      border: preservedAttribute("border"),
      hspace: preservedAttribute("hspace"),
      vspace: preservedAttribute("vspace"),
    };
  },
  parseHTML() {
    return [{ tag: "img" }];
  },
  renderHTML({ node, HTMLAttributes }) {
    const { emailStyle, ...attributes } = HTMLAttributes;
    const source = typeof node.attrs.contentId === "string" && node.attrs.contentId
      ? `cid:${node.attrs.contentId}`
      : node.attrs.src;
    return ["img", mergeAttributes(
      attributes,
      typeof source === "string" ? { src: source } : {},
      typeof emailStyle === "string" ? { style: emailStyle } : {},
    )];
  },
  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement("img");
      dom.contentEditable = "false";
      const update = (current: typeof node) => {
        const preview = typeof current.attrs.previewSrc === "string"
          ? current.attrs.previewSrc
          : typeof current.attrs.src === "string" && current.attrs.src.toLocaleLowerCase().startsWith("data:image/")
            ? current.attrs.src
            : null;
        if (preview) {
          dom.src = preview;
          dom.hidden = false;
        } else {
          dom.removeAttribute("src");
          dom.hidden = true;
        }
        dom.alt = typeof current.attrs.alt === "string" ? current.attrs.alt : "";
        const emailClass = typeof current.attrs.emailClass === "string"
          ? current.attrs.emailClass.trim()
          : "";
        dom.className = ["nextmail-email-image", emailClass].filter(Boolean).join(" ");
        if (typeof current.attrs.emailId === "string" && current.attrs.emailId) {
          dom.id = current.attrs.emailId;
        } else {
          dom.removeAttribute("id");
        }
        for (const attribute of [
          "title",
          "width",
          "height",
          "align",
          "border",
          "hspace",
          "vspace",
        ] as const) {
          const value = current.attrs[attribute];
          if ((typeof value === "string" || typeof value === "number") && `${value}`) {
            dom.setAttribute(attribute, `${value}`);
          }
          else dom.removeAttribute(attribute);
        }
        dom.style.cssText = typeof current.attrs.emailStyle === "string"
          ? current.attrs.emailStyle
          : "";
      };
      update(node);
      return {
        dom,
        update(current) {
          if (current.type.name !== "nextmailImage") return false;
          update(current);
          return true;
        },
      };
    };
  },
});

function legacyTableAttributes(names: string[]) {
  return Object.fromEntries(names.map((name) => [name, preservedAttribute(name)]));
}

export const EmailTable = Table.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      ...legacyTableAttributes(["border", "cellpadding", "cellspacing", "bgcolor", "height", "role", "width"]),
    };
  },
}).configure({ resizable: false, allowTableNodeSelection: true });

export const EmailTableRow = TableRow.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      ...legacyTableAttributes(["bgcolor", "height", "valign", "width"]),
    };
  },
});

export const EmailTableCell = TableCell.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      ...legacyTableAttributes(["bgcolor", "height", "nowrap", "valign", "width"]),
    };
  },
});

export const EmailTableHeader = TableHeader.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      ...legacyTableAttributes(["bgcolor", "height", "nowrap", "valign", "width"]),
    };
  },
});
