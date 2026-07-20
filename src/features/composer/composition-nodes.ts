import { mergeAttributes, Node } from "@tiptap/core";

function compositionNode(name: string, dataAttribute: string) {
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
      const kind = name === "nextmailTemplate" ? "template" : "signature";
      return ["div", mergeAttributes(HTMLAttributes, { class: `nextmail-composition-${kind}` }), 0];
    },
  });
}

export const NextMailTemplate = compositionNode(
  "nextmailTemplate",
  "data-nextmail-template-id",
);

export const NextMailSignature = compositionNode(
  "nextmailSignature",
  "data-nextmail-signature-id",
);
