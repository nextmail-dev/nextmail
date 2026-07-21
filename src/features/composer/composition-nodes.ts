import { mergeAttributes, Node } from "@tiptap/core";

import {
  buildComposerPreviewDocument,
  estimateComposerDocumentHeight,
} from "./composer-html";

function definitionNode(name: string, dataAttribute: string, kind: string) {
  return Node.create({
    name,
    group: "block",
    content: "block+",
    defining: true,
    addAttributes() {
      return {
        definitionId: {
          default: null,
          parseHTML: (element) => element.getAttribute(dataAttribute),
          renderHTML: (attributes) => attributes.definitionId
            ? { [dataAttribute]: attributes.definitionId }
            : {},
        },
      };
    },
    parseHTML() {
      return [{ tag: `div[${dataAttribute}]` }];
    },
    renderHTML({ HTMLAttributes }) {
      return ["div", mergeAttributes(HTMLAttributes, { class: `nextmail-composition-${kind}` }), 0];
    },
  });
}

export const NextMailTemplate = definitionNode(
  "nextmailTemplate",
  "data-nextmail-template-id",
  "template",
);

export const NextMailSignature = definitionNode(
  "nextmailSignature",
  "data-nextmail-signature-id",
  "signature",
);

export const NextMailReply = Node.create({
  name: "nextmailReply",
  group: "block",
  content: "block+",
  defining: true,
  parseHTML() {
    return [{ tag: "div[data-nextmail-reply]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, {
      class: "nextmail-composition-reply",
      "data-nextmail-reply": "",
    }), 0];
  },
});

export function createNextMailOriginalMessage(
  getPreviews: () => Record<string, string>,
  previewTitle: string,
) {
  return Node.create({
    name: "nextmailOriginalMessage",
    group: "block",
    atom: true,
    defining: true,
    isolating: true,
    addAttributes() {
      return {
        sourceHtml: {
          default: "",
          rendered: false,
          parseHTML: (element) => element.innerHTML,
        },
        sourcePlainText: {
          default: "",
          rendered: false,
          parseHTML: (element) => element.textContent ?? "",
        },
      };
    },
    parseHTML() {
      return [{ tag: "div[data-nextmail-original-message]" }];
    },
    renderHTML({ HTMLAttributes }) {
      return ["div", mergeAttributes(HTMLAttributes, {
        class: "nextmail-composition-original-message",
        "data-nextmail-original-message": "",
      })];
    },
    addNodeView() {
      return ({ node }) => {
        const dom = document.createElement("div");
        dom.className = "nextmail-composition-original-message";
        dom.contentEditable = "false";
        const frame = document.createElement("iframe");
        frame.className = "nextmail-composition-original-frame";
        frame.title = previewTitle;
        frame.tabIndex = -1;
        frame.setAttribute("sandbox", "");
        frame.setAttribute("scrolling", "no");
        frame.referrerPolicy = "no-referrer";
        const update = (current: typeof node) => {
          const sourceHtml = typeof current.attrs.sourceHtml === "string"
            ? current.attrs.sourceHtml
            : "";
          const sourcePlainText = typeof current.attrs.sourcePlainText === "string"
            ? current.attrs.sourcePlainText
            : "";
          const previews = getPreviews();
          frame.srcdoc = buildComposerPreviewDocument(
            sourceHtml,
            previews,
          );
          frame.style.height = `${estimateComposerDocumentHeight(sourceHtml, sourcePlainText, previews)}px`;
        };
        update(node);
        dom.append(frame);
        return {
          dom,
          update(current) {
            if (current.type.name !== "nextmailOriginalMessage") return false;
            update(current);
            return true;
          },
        };
      };
    },
  });
}
