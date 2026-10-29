import type { Editor } from "@tiptap/core";
import { useEditorState } from "@tiptap/react";
import { useTranslation } from "react-i18next";
import { useEffect, useState, type ReactNode } from "react";
import { Bold, Code2, Highlighter, ImagePlus, Italic, Link2, List, ListOrdered, Palette, Quote, Redo2, Strikethrough, UnderlineIcon, Undo2, Unlink } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EditorFontPicker, EditorFontSize } from "./EditorFontControls";
import { EditorColorPicker } from "./EditorColorPicker";
import { readActiveFormat, readSelectionFormat, toggleInlineFormat } from "./editor-formatting";

interface EditorToolbarProps {
  editor: Editor; disabled?: boolean; sourceMode: boolean; onEdit: (run: () => void) => void;
  onLink: () => void; onRemoveLink: () => void; onImage?: () => void; onSource: () => void;
}

export function EditorToolbar(props: EditorToolbarProps) {
  const { editor } = props;
  const [readyEditor, setReadyEditor] = useState<Editor | null>(null);
  useEffect(() => {
    let active = true;
    const reveal = () => {
      // Tiptap emits create before setting isInitialized. Wait until that
      // lifecycle step completes before reading computed styles from its view.
      queueMicrotask(() => {
        if (active && editor.isInitialized && !editor.isDestroyed) setReadyEditor(editor);
      });
    };
    const hide = () => setReadyEditor(null);
    editor.on("create", reveal).on("unmount", hide).on("destroy", hide);
    reveal();
    return () => {
      active = false;
      editor.off("create", reveal).off("unmount", hide).off("destroy", hide);
    };
  }, [editor]);
  return readyEditor === editor ? <ReadyEditorToolbar {...props} /> : null;
}

function ReadyEditorToolbar({ editor, disabled, sourceMode, onEdit, onLink, onRemoveLink, onImage, onSource }: EditorToolbarProps) {
  const { t } = useTranslation();
  const state = useEditorState({ editor, selector: ({ editor }) => ({
    format: readSelectionFormat(editor),
    quote: readActiveFormat(editor, "blockquote"), bullet: readActiveFormat(editor, "bulletList"), ordered: readActiveFormat(editor, "orderedList"), link: readActiveFormat(editor, "link"),
    undo: editor.can().undo(), redo: editor.can().redo(),
  }) });
  const richDisabled = disabled || sourceMode;
  const action = (key: string, icon: ReactNode, run: () => void, active?: boolean | null, unavailable = false) => <Button type="button" size="icon" className="size-8" variant={active ? "secondary" : "ghost"} title={t(`composer.${key}`)} aria-label={t(`composer.${key}`)} aria-pressed={active === null ? "mixed" : active} disabled={richDisabled || unavailable} onMouseDown={event => event.preventDefault()} onClick={() => onEdit(run)}>{icon}</Button>;
  const group = (label: string, children: ReactNode) => <div role="group" aria-label={t(`composer.${label}`)} className="flex shrink-0 flex-wrap items-center gap-0.5 border-r border-border/70 pr-2 last:border-r-0 last:pr-0">{children}</div>;
  return <div role="toolbar" aria-label={t("composer.formattingToolbar")} className="flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1 border-b border-border/70 bg-muted/25 px-3 py-2">
    {group("historyTools", <>
      {action("undo", <Undo2 size={16} />, () => editor.chain().focus().undo().run(), undefined, !state.undo)}
      {action("redo", <Redo2 size={16} />, () => editor.chain().focus().redo().run(), undefined, !state.redo)}
    </>)}
    {group("fontTools", <div className="flex items-center gap-1.5">
      <EditorFontPicker value={state.format.fontFamily} disabled={richDisabled} onSelect={value => onEdit(() => { if (value) editor.chain().focus().setFontFamily(value).run(); else editor.chain().focus().unsetFontFamily().run(); })} />
      <EditorFontSize value={state.format.fontSize} disabled={richDisabled} onSelect={value => onEdit(() => { if (value) editor.chain().focus().setFontSize(value).run(); else editor.chain().focus().unsetFontSize().run(); })} />
    </div>)}
    {group("textTools", <>
      {action("bold", <Bold size={16} />, () => toggleInlineFormat(editor, "bold", state.format), state.format.bold)}
      {action("italic", <Italic size={16} />, () => toggleInlineFormat(editor, "italic", state.format), state.format.italic)}
      {action("underline", <UnderlineIcon size={16} />, () => toggleInlineFormat(editor, "underline", state.format), state.format.underline)}
      {action("strike", <Strikethrough size={16} />, () => toggleInlineFormat(editor, "strike", state.format), state.format.strike)}
      <EditorColorPicker label={t("composer.textColor")} icon={<Palette size={16} />} value={state.format.color} disabled={richDisabled} onSelect={value => onEdit(() => { if (value) editor.chain().focus().setColor(value).run(); else editor.chain().focus().unsetColor().run(); })} />
      <EditorColorPicker label={t("composer.backgroundColor")} icon={<Highlighter size={16} />} value={state.format.backgroundColor} disabled={richDisabled} onSelect={value => onEdit(() => { if (value) editor.chain().focus().setBackgroundColor(value).run(); else editor.chain().focus().unsetBackgroundColor().run(); })} />
    </>)}
    {group("paragraphTools", <>
      {action("bulletList", <List size={16} />, () => editor.chain().focus().toggleBulletList().run(), state.bullet)}
      {action("numberedList", <ListOrdered size={16} />, () => editor.chain().focus().toggleOrderedList().run(), state.ordered)}
      {action("quote", <Quote size={16} />, () => editor.chain().focus().toggleBlockquote().run(), state.quote)}
    </>)}
    {group("insertTools", <>
      {action("link", <Link2 size={16} />, onLink, state.link)}
      {action("removeLink", <Unlink size={16} />, onRemoveLink, undefined, state.link === false)}
      {onImage ? action("insertImage", <ImagePlus size={16} />, onImage) : null}
      <Button type="button" variant={sourceMode ? "secondary" : "ghost"} className="h-8 gap-1.5 px-2" aria-label={t("composer.htmlSource")} aria-pressed={sourceMode} disabled={disabled} onMouseDown={event => event.preventDefault()} onClick={onSource}><Code2 size={16} /><span className="text-xs">HTML</span></Button>
    </>)}
  </div>;
}
