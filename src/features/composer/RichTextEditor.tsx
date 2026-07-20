import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import { TextStyleKit } from "@tiptap/extension-text-style";
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
  Palette,
  Highlighter,
} from "lucide-react";
import { useEffect } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { DraftContent } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Inline, Page } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Text } from "@/components/ui/typography";
import { cn } from "@/lib/utils";

interface RichTextEditorProps {
  initialJson: string;
  disabled?: boolean;
  signature?: { name: string; email: string };
  ariaLabel?: string;
  className?: string;
  onChange: (content: DraftContent) => void;
}

export function RichTextEditor({ initialJson, disabled, signature, ariaLabel, className, onChange }: RichTextEditorProps) {
  const { t } = useTranslation();
  const editor = useEditor({
    extensions: [StarterKit.configure({ underline: false }), Underline, TextStyleKit],
    content: parseDocument(initialJson),
    editable: !disabled,
    editorProps: {
      attributes: {
        class: "nextmail-editor-content",
        "aria-label": ariaLabel ?? t("composer.body"),
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
    <Page className={cn("flex min-h-0 flex-1 flex-col bg-card", className)}>
      <Inline className="min-h-11 shrink-0 gap-0.5 overflow-x-auto bg-muted/35 px-3 py-1.5" role="toolbar">
        <SelectField
          compact
          className="shrink-0"
          triggerClassName="min-w-32 border-transparent"
          label={t("composer.fontFamily")}
          value={editor.getAttributes("textStyle").fontFamily ?? "default"}
          options={[
            { value: "default", label: t("composer.fontDefault") },
            { value: "Arial, sans-serif", label: "Arial" },
            { value: "Georgia, serif", label: "Georgia" },
            { value: "'Courier New', monospace", label: "Courier New" },
            { value: "'Microsoft YaHei', sans-serif", label: "微软雅黑" },
          ]}
          disabled={disabled}
          onValueChange={(value) => value === "default"
            ? editor.chain().focus().unsetFontFamily().run()
            : editor.chain().focus().setFontFamily(value).run()}
        />
        <SelectField
          compact
          className="shrink-0"
          triggerClassName="min-w-20"
          label={t("composer.fontSize")}
          value={editor.getAttributes("textStyle").fontSize ?? "default"}
          options={[
            { value: "default", label: t("composer.fontSizeDefault") },
            { value: "12px", label: "12" },
            { value: "14px", label: "14" },
            { value: "16px", label: "16" },
            { value: "18px", label: "18" },
            { value: "24px", label: "24" },
            { value: "32px", label: "32" },
          ]}
          disabled={disabled}
          onValueChange={(value) => value === "default"
            ? editor.chain().focus().unsetFontSize().run()
            : editor.chain().focus().setFontSize(value).run()}
        />
        <ColorMenu
          label={t("composer.textColor")}
          icon={<Palette size={16} />}
          disabled={disabled}
          onSelect={(value) => value ? editor.chain().focus().setColor(value).run() : editor.chain().focus().unsetColor().run()}
        />
        <ColorMenu
          label={t("composer.backgroundColor")}
          icon={<Highlighter size={16} />}
          disabled={disabled}
          background
          onSelect={(value) => value ? editor.chain().focus().setBackgroundColor(value).run() : editor.chain().focus().unsetBackgroundColor().run()}
        />
        <span className="mx-1 h-5 w-px shrink-0 bg-border" aria-hidden="true" />
        {action(t("composer.bold"), editor.isActive("bold"), () => editor.chain().focus().toggleBold().run(), <Bold size={16} />)}
        {action(t("composer.italic"), editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run(), <Italic size={16} />)}
        {action(t("composer.underline"), editor.isActive("underline"), () => editor.chain().focus().toggleUnderline().run(), <UnderlineIcon size={16} />)}
        {action(t("composer.strike"), editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run(), <Strikethrough size={16} />)}
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        {action(t("composer.bulletList"), editor.isActive("bulletList"), () => editor.chain().focus().toggleBulletList().run(), <List size={16} />)}
        {action(t("composer.numberedList"), editor.isActive("orderedList"), () => editor.chain().focus().toggleOrderedList().run(), <ListOrdered size={16} />)}
        {action(t("composer.quote"), editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run(), <Quote size={16} />)}
        {signature ? action(t("composer.insertSignature"), false, () => editor.chain().focus().insertContent([
          { type: "paragraph", content: [{ type: "text", text: "-- " }] },
          { type: "paragraph", content: [{ type: "text", text: signature.name || signature.email }] },
          ...(signature.name ? [{ type: "paragraph", content: [{ type: "text", text: signature.email }] }] : []),
        ]).run(), <UserRound size={16} />) : null}
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        {action(t("composer.undo"), false, () => editor.chain().focus().undo().run(), <Undo2 size={16} />)}
        {action(t("composer.redo"), false, () => editor.chain().focus().redo().run(), <Redo2 size={16} />)}
      </Inline>
      <EditorContent editor={editor} className="min-h-0 flex-1 overflow-auto" />
    </Page>
  );
}

function ColorMenu({ label, icon, disabled, background, onSelect }: {
  label: string;
  icon: ReactNode;
  disabled?: boolean;
  background?: boolean;
  onSelect: (value: string | null) => void;
}) {
  const { t } = useTranslation();
  const colors = background
    ? [null, "#fff2a8", "#ffd8a8", "#c8f7d5", "#cfe3ff", "#ead7ff", "#ffd6e7"]
    : [null, "#202124", "#c93737", "#b45f06", "#18734d", "#2563eb", "#7c3aed", "#d12f7a"];
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button type="button" variant="ghost" size="icon" disabled={disabled} aria-label={label} title={label}>{icon}</Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-48">
        {colors.map((color) => (
          <DropdownMenuItem key={color ?? "default"} onSelect={() => onSelect(color)}>
            <span
              className="size-4 rounded-xs ring-1 ring-border"
              style={color ? { backgroundColor: color } : undefined}
              aria-hidden="true"
            />
            <Text className="text-[length:var(--ui-font-control)] text-foreground">
              {color ? color.toUpperCase() : t("composer.colorDefault")}
            </Text>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function parseDocument(value: string) {
  try {
    return JSON.parse(value) as Record<string, unknown>;
  } catch {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
}
