import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Editor } from "@tiptap/core";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import i18n from "@/app/i18n";
import { api } from "@/app/api";
import { RichTextEditor } from "./RichTextEditor";
import { parseFontSize, readSelectionFormat } from "./editor-formatting";
import { colorToHex, hexToHsv, hsvToHex } from "./EditorColorPicker";

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
  // jsdom has no layout; ProseMirror's deferred focus still queries range geometry.
  Object.defineProperties(Range.prototype, {
    getClientRects: { configurable: true, value: () => [] },
    getBoundingClientRect: { configurable: true, value: () => new DOMRect() },
  });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

async function setup(content: object[], initialHtml?: string) {
  const onChange = vi.fn();
  const result = render(<RichTextEditor initialJson={JSON.stringify({ type: "doc", content })} initialHtml={initialHtml} onChange={onChange} />);
  const element = await waitFor(() => {
    const node = result.container.querySelector<HTMLElement & { editor: Editor }>(".tiptap");
    expect(node?.editor).toBeDefined(); return node!;
  });
  return { ...result, element, editor: element.editor, onChange };
}
const line = (text: string) => ({ type: "paragraph", content: text ? [{ type: "text", text }] : [] });
function position(editor: Editor, text: string) {
  let found = 1;
  editor.state.doc.descendants((node, pos) => { if (node.isText && node.text === text) found = pos + 1; });
  return found;
}

describe("rich editor interaction", () => {
  it("exposes the former more actions and follows cursor and mixed selection formatting", async () => {
    const { editor } = await setup([{ type: "paragraph", content: [
      { type: "text", text: "Large", marks: [{ type: "textStyle", attrs: { fontSize: "28px", fontFamily: "Georgia", color: "#123456" } }, { type: "bold" }] },
      { type: "text", text: "Small" },
    ] }]);
    expect(screen.queryByRole("button", { name: "More formatting options" })).toBeNull();
    for (const label of ["Quote", "Insert or edit link", "Remove link", "HTML source"]) expect(screen.getByRole("button", { name: label })).toBeVisible();
    act(() => { editor.commands.setTextSelection(3); });
    expect(screen.getByRole("button", { name: "Bold" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("textbox", { name: "Font size" })).toHaveValue("28");
    expect(screen.getByRole("button", { name: "Font family" })).toHaveTextContent("Georgia");
    act(() => { editor.commands.setTextSelection(9); });
    expect(screen.getByRole("button", { name: "Bold" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("textbox", { name: "Font size" })).toHaveValue("14");
    act(() => { editor.commands.setTextSelection({ from: 1, to: 11 }); });
    expect(screen.getByRole("button", { name: "Bold" })).toHaveAttribute("aria-pressed", "mixed");
    expect(screen.getByRole("textbox", { name: "Font size" })).toHaveValue("");
  });

  it("applies editable point sizes and custom colors to the retained selection and supports undo", async () => {
    const { editor } = await setup([line("Hello world")]);
    act(() => { editor.commands.setTextSelection({ from: 1, to: 6 }); });
    const size = screen.getByRole("textbox", { name: "Font size" });
    fireEvent.focus(size); fireEvent.change(size, { target: { value: "17.5pt" } }); fireEvent.keyDown(size, { key: "Enter" });
    await act(async () => { await new Promise(requestAnimationFrame); });
    expect(editor.getHTML()).toContain("font-size: 17.5pt");
    expect(editor.state.selection.from).toBe(1); expect(editor.state.selection.to).toBe(6);
    fireEvent.click(screen.getByRole("button", { name: "Text color" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Color value (HEX / RGB)" }), { target: { value: "#a13fe7" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply color" }));
    expect(editor.getHTML()).toContain("rgb(161, 63, 231)");
    expect(editor.getHTML()).toMatch(/Hello<\/span> world/);
    expect(screen.getByRole("button", { name: "Undo" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(editor.getHTML()).not.toContain("rgb(161, 63, 231)");
  });

  it("previews installed fonts and applies a searched family", async () => {
    vi.spyOn(api, "listSystemFonts").mockResolvedValue(["Arial", "Georgia", "测试字体"]);
    const { editor } = await setup([line("Hello")]);
    act(() => { editor.commands.setTextSelection({ from: 1, to: 6 }); });
    fireEvent.click(screen.getByRole("button", { name: "Font family" }));
    fireEvent.change(await screen.findByRole("combobox", { name: "Search system fonts" }), { target: { value: "测试" } });
    const option = await screen.findByRole("option", { name: /测试字体/ });
    expect(option.querySelector("span span")).toHaveStyle({ fontFamily: '"测试字体"' });
    fireEvent.click(option);
    expect(editor.getAttributes("textStyle").fontFamily).toBe('"测试字体"');
  });

  it("removes a nested empty visual line with one Backspace, without touching text or original mail", async () => {
    const { editor, element } = await setup([line("Keep"), { type: "emailBlock", content: [{ type: "emailBlock", content: [line("")] }] },
      { type: "nextmailOriginalMessage", attrs: { sourceHtml: "<p>Original</p>", sourcePlainText: "Original" } }]);
    let emptyPos = 0;
    editor.state.doc.descendants((node, pos) => { if (node.type.name === "paragraph" && node.content.size === 0) emptyPos = pos + 1; });
    act(() => { editor.commands.setTextSelection(emptyPos); });
    fireEvent.keyDown(element, { key: "Backspace" });
    expect(editor.getJSON().content?.map(node => node.type)).toEqual(["paragraph", "nextmailOriginalMessage"]);
    expect(editor.getText()).toContain("Keep");
    expect(editor.getJSON().content?.[1].attrs?.sourceHtml).toBe("<p>Original</p>");
    act(() => { editor.commands.undo(); });
    expect(editor.getJSON().content?.map(node => node.type)).toEqual(["paragraph", "emailBlock", "nextmailOriginalMessage"]);
  });

  it("does not regenerate an empty trailing line when deleting after an imported div", async () => {
    const { editor, element } = await setup([{ type: "emailInlineBlock", content: [{ type: "text", text: "Keep" }] }, line("")]);
    act(() => { editor.commands.setTextSelection(editor.state.doc.content.size - 1); });
    fireEvent.keyDown(element, { key: "Backspace" });
    expect(editor.getJSON().content).toHaveLength(1);
    expect(editor.getText()).toBe("Keep");
  });

  it("persists Backspace changes in a reopened draft and removes empty wrappers containing scoped styles", async () => {
    const { editor, element, onChange } = await setup([line("Keep"), { type: "emailBlock", content: [
      { type: "emailStylesheet", attrs: { css: "[data-nextmail-pasted-html] p { color: red }" } },
      { type: "emailBlock", content: [line("")] },
    ] }], "<p>Keep</p><div><p></p></div>");
    expect(onChange).not.toHaveBeenCalled();
    let emptyPos = 0;
    editor.state.doc.descendants((node, pos) => { if (node.type.name === "paragraph" && !node.content.size) emptyPos = pos + 1; });
    act(() => { editor.commands.setTextSelection(emptyPos); });
    fireEvent.keyDown(element, { key: "Backspace" });
    expect(editor.getJSON().content).toHaveLength(1);
    expect(onChange).toHaveBeenCalled();
    expect(onChange.mock.lastCall?.[0].plainText.trim()).toBe("Keep");
  });

  it("reflects cleared stored styles for the next character and cancels size edits with Escape", async () => {
    const { editor } = await setup([{ type: "paragraph", content: [
      { type: "text", text: "Styled", marks: [{ type: "textStyle", attrs: { fontSize: "28px", fontFamily: "Georgia", color: "#123456" } }] },
    ] }]);
    act(() => { editor.commands.setTextSelection(4); editor.commands.unsetFontSize(); editor.commands.unsetFontFamily(); editor.commands.unsetColor(); });
    expect(readSelectionFormat(editor).fontSize).toBe("14px");
    expect(readSelectionFormat(editor).fontFamily).not.toBe("Georgia");
    const size = screen.getByRole("textbox", { name: "Font size" });
    act(() => size.focus());
    fireEvent.change(size, { target: { value: "42" } });
    fireEvent.keyDown(size, { key: "Escape" });
    expect(readSelectionFormat(editor).fontSize).toBe("14px");
  });

  it("removes imported underlines only from selected text, preserving the rest and undo", async () => {
    const { editor } = await setup([{ type: "paragraph", attrs: { emailStyle: "text-decoration:underline;color:rgb(10,20,30)" }, content: [
      { type: "text", text: "First second", marks: [{ type: "emailSpan", attrs: { emailStyle: "text-decoration:underline" } }] },
    ] }]);
    act(() => { editor.commands.setTextSelection({ from: 1, to: 6 }); });
    expect(readSelectionFormat(editor).underline).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Underline" }));
    expect(readSelectionFormat(editor).underline).toBe(false);
    act(() => { editor.commands.setTextSelection(10); });
    expect(readSelectionFormat(editor).underline).toBe(true);
    act(() => { editor.commands.undo(); editor.commands.setTextSelection(3); });
    expect(readSelectionFormat(editor).underline).toBe(true);
  });

  it("reflects imported span and paragraph styles and lets a new font override them", async () => {
    const { editor } = await setup([{ type: "paragraph", attrs: { emailStyle: "font-size: 22px; font-family: Georgia; font-weight:700" }, content: [
      { type: "text", text: "Styled", marks: [{ type: "emailSpan", attrs: { emailStyle: "color:#e34f12;font-family:Verdana" } }] },
    ] }]);
    act(() => { editor.commands.setTextSelection(position(editor, "Styled")); });
    expect(readSelectionFormat(editor).fontFamily).toBe("Verdana");
    expect(readSelectionFormat(editor).fontSize).toBe("22px");
    act(() => { editor.commands.setTextSelection({ from: 1, to: 7 }); editor.commands.setFontFamily("Arial"); });
    expect(readSelectionFormat(editor).fontFamily).toBe("Arial");
    fireEvent.click(screen.getByRole("button", { name: "Bold" }));
    expect(readSelectionFormat(editor).bold).toBe(false);
  });

  it("keeps the empty editing surface in the viewport's flex chain", async () => {
    const { container } = await setup([line("")]);
    expect(container.querySelector(".native-scrollbar-hidden")).toHaveClass("flex", "flex-col");
    expect(container.querySelector(".nextmail-editor-surface")).toHaveClass("flex", "flex-1", "flex-col");
  });

  it("clears the final nested blank line and keeps a usable paragraph", async () => {
    const { editor, element } = await setup([{ type: "emailBlock", content: [{ type: "emailBlock", content: [line("")] }] }]);
    act(() => { editor.commands.setTextSelection(3); });
    fireEvent.keyDown(element, { key: "Backspace" });
    expect(editor.getJSON().content?.map(node => node.type)).toEqual(["paragraph"]);
    act(() => { editor.commands.insertContent("New text"); });
    expect(editor.getText()).toBe("New text");
  });

  it("shares keyboard and toolbar formatting and reports mixed lists and links", async () => {
    const { editor, element } = await setup([
      { type: "bulletList", content: [{ type: "listItem", content: [line("Item")] }] },
      { type: "paragraph", content: [{ type: "text", text: "Link", marks: [{ type: "link", attrs: { href: "https://example.com" } }] }, { type: "text", text: " plain" }] },
    ]);
    act(() => { editor.commands.selectAll(); });
    expect(screen.getByRole("button", { name: "Bullet list" })).toHaveAttribute("aria-pressed", "mixed");
    expect(screen.getByRole("button", { name: "Insert or edit link" })).toHaveAttribute("aria-pressed", "mixed");
    expect(screen.getByRole("button", { name: "Remove link" })).toBeEnabled();
    act(() => { editor.commands.setTextSelection(position(editor, "Item")); });
    fireEvent.click(screen.getByRole("button", { name: "Bold" }));
    expect(readSelectionFormat(editor).bold).toBe(true);
    fireEvent.keyDown(element, { key: "b", ctrlKey: true });
    expect(readSelectionFormat(editor).bold).toBe(false);
    fireEvent.keyDown(element, { key: "b", ctrlKey: true });
    expect(readSelectionFormat(editor).bold).toBe(true);
  });
});

describe("format input validation", () => {
  it("accepts bounded editable sizes and rejects invalid CSS", () => {
    expect(parseFontSize("17.5pt")).toBe("17.5pt"); expect(parseFontSize(" 24 ")).toBe("24px");
    for (const value of ["5", "145", "-12", "16em", "12px;color:red", "NaN"]) expect(parseFontSize(value)).toBeNull();
  });
  it("round-trips arbitrary colors through the continuous color panel", () => {
    for (const color of ["#000000", "#ffffff", "#a13fe7", "#2c9d31"]) { const hsv = hexToHsv(color); expect(hsvToHex(hsv.h, hsv.s, hsv.v)).toBe(color); }
    expect(colorToHex("rgb(161, 63, 231)")).toBe("#a13fe7");
    expect(colorToHex("#abc")).toBe("#aabbcc");
    expect(colorToHex("invalid")).toBeNull();
    expect(colorToHex("rgb(256, 0, 0)")).toBeNull();
    expect(colorToHex("rgb(1, 2, 3);color:red")).toBeNull();
  });
});
