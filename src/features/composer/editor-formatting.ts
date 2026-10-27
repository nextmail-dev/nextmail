import { Extension, type Editor } from "@tiptap/core";
import type { Mark } from "@tiptap/pm/model";

export const FONT_SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 48, 64, 72, 96];
export const quoteFontFamily = (name: string) => `"${name.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
export const fontLabel = (family: string) => family.split(",")[0].trim().replace(/^["']|["']$/g, "");

export function parseFontSize(value: string): string | null {
  const match = /^\s*(\d+(?:\.\d+)?)\s*(px|pt)?\s*$/i.exec(value);
  if (!match) return null;
  const size = Number(match[1]);
  if (!Number.isFinite(size) || size < 6 || size > 144) return null;
  return `${size}${match[2]?.toLowerCase() ?? "px"}`;
}

export const ComposerTextAttributes = Extension.create({
  name: "composerTextAttributes",
  addGlobalAttributes() {
    return [{ types: ["textStyle"], attributes: Object.fromEntries([
      ["fontWeight", "font-weight"], ["fontStyle", "font-style"], ["textDecoration", "text-decoration"],
    ].map(([name, css]) => [name, {
      default: null,
      parseHTML: (element: HTMLElement) => element.style.getPropertyValue(css) || null,
      renderHTML: (attrs: Record<string, string>) => attrs[name] ? { style: `${css}:${attrs[name]}` } : {},
    }])) }];
  },
});

export interface FormatValues {
  fontFamily: string;
  fontSize: string;
  color: string;
  backgroundColor: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
}
export type SelectionFormat = { [K in keyof FormatValues]: FormatValues[K] | null };

function styleElement(editor: Editor, position: number): HTMLElement {
  const point = editor.view.domAtPos(position);
  const node = point.node.nodeType === Node.TEXT_NODE
    ? point.node
    : point.node.childNodes[point.offset] ?? point.node.childNodes[point.offset - 1] ?? point.node;
  return (node instanceof HTMLElement ? node : node.parentElement) ?? editor.view.dom;
}

function effectiveFormat(editor: Editor, position: number, marks: readonly Mark[], pending = false): FormatValues {
  let element = styleElement(editor, position);
  if (pending) {
    // Stored marks describe the next character, not the character beside the caret.
    const block = editor.view.nodeDOM(editor.state.selection.$from.before());
    if (block instanceof HTMLElement) element = block;
  }
  const style = getComputedStyle(element);
  const attrs: Record<string, string | null> = {};
  for (const mark of marks) {
    const inline = document.createElement("span").style;
    inline.cssText = typeof mark.attrs.emailStyle === "string" ? mark.attrs.emailStyle : "";
    for (const name of ["fontFamily", "fontSize", "color", "backgroundColor", "fontWeight", "fontStyle", "textDecoration"] as const) {
      if (inline[name]) attrs[name] = inline[name];
    }
    if (mark.type.name === "emailFont") {
      if (mark.attrs.face) attrs.fontFamily = mark.attrs.face;
      if (mark.attrs.color) attrs.color = mark.attrs.color;
    }
    if (mark.type.name === "textStyle") {
      for (const [name, value] of Object.entries(mark.attrs)) if (typeof value === "string" && value) attrs[name] = value;
    }
  }
  const has = (name: string) => marks.some(mark => mark.type.name === name);
  let background = "transparent";
  const decorations: string[] = [];
  // Decorations/backgrounds can be painted by ancestors without being inherited.
  for (let current: HTMLElement | null = element; current && current !== editor.view.dom; current = current.parentElement) {
    const computed = getComputedStyle(current);
    decorations.push(computed.textDecorationLine, computed.textDecoration);
    if (background === "transparent" && computed.backgroundColor && !["transparent", "rgba(0, 0, 0, 0)"].includes(computed.backgroundColor)) background = computed.backgroundColor;
  }
  const weight = attrs.fontWeight || style.fontWeight;
  const fontStyle = attrs.fontStyle || style.fontStyle;
  const decoration = attrs.textDecoration ?? decorations.join(" ");
  return {
    fontFamily: attrs.fontFamily || style.fontFamily || "system-ui",
    fontSize: attrs.fontSize || style.fontSize || "14px",
    color: (pending ? attrs.color || style.color : style.color || attrs.color) || "#202124",
    backgroundColor: pending ? attrs.backgroundColor || background : background,
    bold: attrs.fontWeight ? weight === "bold" || Number(weight) >= 600 : has("bold") || weight === "bold" || Number(weight) >= 600,
    italic: attrs.fontStyle ? fontStyle === "italic" || fontStyle === "oblique" : has("italic") || fontStyle === "italic" || fontStyle === "oblique",
    underline: attrs.textDecoration != null ? decoration.includes("underline") : has("underline") || decoration.includes("underline"),
    strike: attrs.textDecoration != null ? decoration.includes("line-through") : has("strike") || decoration.includes("line-through"),
  };
}

export function readSelectionFormat(editor: Editor): SelectionFormat {
  const { selection, storedMarks, doc } = editor.state;
  let result: SelectionFormat | undefined;
  if (selection.empty) {
    const pos = selection.$from.nodeBefore?.isText ? Math.max(0, selection.from - 1) : selection.from;
    result = effectiveFormat(editor, pos, storedMarks ?? selection.$from.marks(), storedMarks !== null && selection.$from.depth > 0);
  } else {
    doc.nodesBetween(selection.from, selection.to, (node, pos) => {
      if (!node.isText) return;
      const values = effectiveFormat(editor, Math.max(selection.from, pos), node.marks);
      if (!result) { result = values; return; }
      for (const key of Object.keys(values) as (keyof FormatValues)[]) {
        if (result[key] !== values[key]) Object.assign(result, { [key]: null });
      }
    });
  }
  return result ?? effectiveFormat(editor, selection.from, []);
}

export function readActiveFormat(editor: Editor, name: string): boolean | null {
  const { selection, doc } = editor.state;
  if (selection.empty) return editor.isActive(name);
  const values = new Set<boolean>();
  doc.nodesBetween(selection.from, selection.to, (node, pos) => {
    if (editor.schema.marks[name] && node.isText) values.add(node.marks.some(mark => mark.type.name === name));
    if (!editor.schema.nodes[name] || !node.isTextblock) return;
    const resolved = doc.resolve(pos + 1);
    let active = false;
    for (let depth = resolved.depth; depth > 0; depth--) if (resolved.node(depth).type.name === name) active = true;
    values.add(active);
  });
  return values.size > 1 ? null : values.values().next().value ?? editor.isActive(name);
}

export function toggleInlineFormat(editor: Editor, name: "bold" | "italic" | "underline" | "strike", current: SelectionFormat) {
  const enabled = current[name] !== true;
  let chain = editor.chain().focus();
  if (name === "bold") chain = chain.unsetBold().setMark("textStyle", { fontWeight: enabled ? "700" : "400" });
  if (name === "italic") chain = chain.unsetItalic().setMark("textStyle", { fontStyle: enabled ? "italic" : "normal" });
  if (name === "underline" || name === "strike") {
    const underline = name === "underline" ? enabled : current.underline;
    const strike = name === "strike" ? enabled : current.strike;
    chain = chain.command(({ tr }) => {
      const { from, to } = tr.selection;
      // CSS decorations painted by a parent cannot be cancelled on a child span.
      // Move them onto text marks first, preserving unselected text and other styles.
      tr.doc.nodesBetween(from, to, (node, pos) => {
        if (!node.isBlock || node.isAtom || !("emailStyle" in node.attrs)) return;
        const dom = editor.view.nodeDOM(pos);
        if (!(dom instanceof HTMLElement)) return;
        const computed = getComputedStyle(dom);
        const decoration = `${computed.textDecorationLine} ${computed.textDecoration}`;
        if (!/underline|line-through/.test(decoration)) return;
        const css = document.createElement("span").style;
        css.cssText = node.attrs.emailStyle ?? "";
        css.textDecoration = "none";
        tr.setNodeMarkup(pos, undefined, { ...node.attrs, emailStyle: css.cssText });
        node.descendants((child, offset) => {
          if (!child.isText) return;
          for (const [token, type] of [["underline", "underline"], ["line-through", "strike"]]) {
            if (decoration.includes(token)) tr.addMark(pos + 1 + offset, pos + 1 + offset + child.nodeSize, editor.schema.marks[type].create());
          }
        });
      });
      const clean = (mark: Mark) => {
        if (!["emailSpan", "emailFont"].includes(mark.type.name)) return mark;
        const css = document.createElement("span").style;
        css.cssText = mark.attrs.emailStyle ?? "";
        css.textDecoration = "none";
        css.removeProperty("text-decoration-line");
        return mark.type.create({ ...mark.attrs, emailStyle: css.cssText });
      };
      if (tr.selection.empty) tr.setStoredMarks((tr.storedMarks ?? tr.selection.$from.marks()).map(clean));
      else tr.doc.nodesBetween(from, to, (node, pos) => {
        if (!node.isText) return;
        for (const mark of node.marks) {
          const replacement = clean(mark);
          if (replacement === mark) continue;
          const start = Math.max(from, pos), end = Math.min(to, pos + node.nodeSize);
          tr.removeMark(start, end, mark).addMark(start, end, replacement);
        }
      });
      return true;
    }).unsetUnderline().unsetStrike().setMark("textStyle", { textDecoration: [underline && "underline", strike && "line-through"].filter(Boolean).join(" ") || "none" });
  }
  return chain.run();
}
