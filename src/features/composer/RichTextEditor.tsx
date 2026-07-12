import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import {
  Bold,
  Italic,
  List,
  ListOrdered,
  Quote,
  Redo2,
  Strikethrough,
  UnderlineIcon,
  Undo2,
  UserRound,
} from "lucide-react";
import { useEffect } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { DraftContent } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Inline, Page } from "@/components/ui/layout";

interface RichTextEditorProps {
  initialJson: string;
  disabled?: boolean;
  signature: { name: string; email: string };
  onChange: (content: DraftContent) => void;
}

export function RichTextEditor({ initialJson, disabled, signature, onChange }: RichTextEditorProps) {
  const { t } = useTranslation();
  const editor = useEditor({
    extensions: [StarterKit.configure({ underline: false }), Underline],
    content: parseDocument(initialJson),
    editable: !disabled,
    editorProps: {
      attributes: {
        class: "nextmail-editor-content",
        "aria-label": t("composer.body"),
      },
    },
    onUpdate: ({ editor: current }) => {
      onChange({
        editorJson: JSON.stringify(current.getJSON()),
        html: current.getHTML(),
        plainText: current.getText({ blockSeparator: "\n\n" }),
      });
    },
  });

  useEffect(() => {
    editor?.setEditable(!disabled);
  }, [disabled, editor]);

  if (!editor) return null;
  const action = (label: string, active: boolean, onClick: () => void, icon: ReactNode) => (
    <Button
      type="button"
      size="icon"
      variant={active ? "secondary" : "ghost"}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      {icon}
    </Button>
  );

  return (
    <Page className="flex min-h-0 flex-1 flex-col bg-card">
      <Inline className="h-11 shrink-0 gap-0.5 border-b border-border px-3" role="toolbar">
        {action(t("composer.bold"), editor.isActive("bold"), () => editor.chain().focus().toggleBold().run(), <Bold size={16} />)}
        {action(t("composer.italic"), editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run(), <Italic size={16} />)}
        {action(t("composer.underline"), editor.isActive("underline"), () => editor.chain().focus().toggleUnderline().run(), <UnderlineIcon size={16} />)}
        {action(t("composer.strike"), editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run(), <Strikethrough size={16} />)}
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        {action(t("composer.bulletList"), editor.isActive("bulletList"), () => editor.chain().focus().toggleBulletList().run(), <List size={16} />)}
        {action(t("composer.numberedList"), editor.isActive("orderedList"), () => editor.chain().focus().toggleOrderedList().run(), <ListOrdered size={16} />)}
        {action(t("composer.quote"), editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run(), <Quote size={16} />)}
        {action(t("composer.insertSignature"), false, () => editor.chain().focus().insertContent([
          { type: "paragraph", content: [{ type: "text", text: "-- " }] },
          { type: "paragraph", content: [{ type: "text", text: signature.name || signature.email }] },
          ...(signature.name ? [{ type: "paragraph", content: [{ type: "text", text: signature.email }] }] : []),
        ]).run(), <UserRound size={16} />)}
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        {action(t("composer.undo"), false, () => editor.chain().focus().undo().run(), <Undo2 size={16} />)}
        {action(t("composer.redo"), false, () => editor.chain().focus().redo().run(), <Redo2 size={16} />)}
      </Inline>
      <EditorContent editor={editor} className="min-h-0 flex-1 overflow-auto" />
    </Page>
  );
}

function parseDocument(value: string) {
  try {
    return JSON.parse(value) as Record<string, unknown>;
  } catch {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
}
