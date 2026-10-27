import { Extension } from "@tiptap/core";
import { TextSelection } from "@tiptap/pm/state";
import { readSelectionFormat, toggleInlineFormat } from "./editor-formatting";

const WRAPPERS = new Set(["emailBlock", "nextmailReply", "nextmailTemplate", "nextmailSignature"]);

export const ComposerEditing = Extension.create({
  name: "composerEditing",
  priority: 1100,
  addKeyboardShortcuts() {
    const toggle = (name: "bold" | "italic" | "underline" | "strike") => this.editor.isEditable
      && toggleInlineFormat(this.editor, name, readSelectionFormat(this.editor));
    return {
      "Mod-b": () => toggle("bold"),
      "Mod-i": () => toggle("italic"),
      "Mod-u": () => toggle("underline"),
      "Mod-Shift-s": () => toggle("strike"),
      Backspace: () => {
        if (!this.editor.isEditable || this.editor.view.composing) return false;
        const { state, view } = this.editor;
        const { $from, empty } = state.selection;
        if (!empty || !$from.parent.isTextblock || $from.parent.content.size !== 0) return false;
        // Delete an empty visual line together with its otherwise empty wrappers.
        // ProseMirror's default joinBackward lifts one wrapper per key press.
        let depth = $from.depth;
        while (depth > 1) {
          const parent = $from.node(depth - 1);
          let visibleChildren = 0;
          parent.forEach(child => { if (child.type.name !== "emailStylesheet") visibleChildren++; });
          if (!WRAPPERS.has(parent.type.name) || visibleChildren !== 1) break;
          depth--;
        }
        if (depth === $from.depth) return false;
        const from = $from.before(depth);
        const to = $from.after(depth);
        const tr = depth === 1 && state.doc.childCount === 1
          ? state.tr.replaceWith(from, to, state.schema.nodes.paragraph.create())
          : state.tr.delete(from, to);
        tr.setSelection(TextSelection.near(tr.doc.resolve(Math.min(from, tr.doc.content.size)), -1));
        view.dispatch(tr.scrollIntoView());
        return true;
      },
    };
  },
});
