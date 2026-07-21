import { EditorContent, useEditor } from "@tiptap/react";
import { generateJSON } from "@tiptap/core";
import type { Editor, Extensions, JSONContent } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import { TextStyleKit } from "@tiptap/extension-text-style";
import {
  Bold,
  Code2,
  Italic,
  List,
  ListOrdered,
  Quote,
  Redo2,
  Strikethrough,
  UnderlineIcon,
  Undo2,
  Palette,
  Highlighter,
} from "lucide-react";
import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { DraftAttachmentSummary, DraftContent } from "@/app/types";
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
import {
  createNextMailOriginalMessage,
  NextMailReply,
  NextMailSignature,
  NextMailTemplate,
} from "./composition-nodes";
import {
  buildComposerPreviewDocument,
  htmlToPlainText,
  inlineImagePreviews,
} from "./composer-html";
import { HtmlSourceEditor } from "./HtmlSourceEditor";
import {
  EmailBlock,
  EmailFont,
  EmailFormattingAttributes,
  EmailStylesheet,
  EmailTable,
  EmailTableCell,
  EmailTableHeader,
  EmailTableRow,
  NextMailImage,
} from "./email-content-nodes";

const BASE_COMPOSER_EXTENSIONS: Extensions = [
  StarterKit.configure({
    underline: false,
    link: {
      openOnClick: false,
      autolink: false,
      linkOnPaste: false,
    },
    trailingNode: {
      notAfter: ["nextmailOriginalMessage"],
    },
  }),
  Underline,
  TextStyleKit,
  EmailFormattingAttributes,
  EmailStylesheet,
  EmailBlock,
  EmailFont,
  EmailTable,
  EmailTableRow,
  EmailTableCell,
  EmailTableHeader,
  NextMailImage,
  NextMailTemplate,
  NextMailSignature,
  NextMailReply,
];

interface RichTextEditorProps {
  initialJson: string;
  disabled?: boolean;
  ariaLabel?: string;
  className?: string;
  onChange: (content: DraftContent) => void;
  onCompositionChange?: (selection: CompositionNodeSelection) => void;
  inlineImages?: DraftAttachmentSummary[];
  onAddInlineImage?: (file: File) => Promise<DraftAttachmentSummary>;
}

export interface CompositionNodeSelection {
  templateId: string | null;
  signatureId: string | null;
}

export interface RichTextEditorHandle {
  replaceTemplate: (definitionId: string | null, content?: DraftContent) => boolean;
  replaceSignature: (definitionId: string | null, content?: DraftContent) => boolean;
}

export const RichTextEditor = forwardRef<RichTextEditorHandle, RichTextEditorProps>(function RichTextEditor(
  {
    initialJson,
    disabled,
    ariaLabel,
    className,
    onChange,
    onCompositionChange,
    inlineImages = [],
    onAddInlineImage,
  },
  ref,
) {
  const { t } = useTranslation();
  const inlineImagesRef = useRef(inlineImages);
  inlineImagesRef.current = inlineImages;
  const previewMap = useMemo(() => inlineImagePreviews(inlineImages), [inlineImages]);
  const extensions = useMemo<Extensions>(() => [
    ...BASE_COMPOSER_EXTENSIONS,
    createNextMailOriginalMessage(
      () => inlineImagePreviews(inlineImagesRef.current),
      t("composer.htmlPreview"),
    ),
  ], [t]);
  const [sourceMode, setSourceMode] = useState(false);
  const [sourceHtml, setSourceHtml] = useState("");
  const editor = useEditor({
    extensions,
    content: hydrateInlineImagePreviews(parseDocument(initialJson), previewMap),
    editable: !disabled,
    editorProps: {
      attributes: {
        class: "nextmail-editor-content",
        "aria-label": ariaLabel ?? t("composer.body"),
      },
      handlePaste: (view, event) => {
        if (!onAddInlineImage || disabled) return false;
        const clipboard = event.clipboardData;
        const images = Array.from(clipboard?.files ?? [])
          .filter((file) => file.type.toLocaleLowerCase().startsWith("image/"));
        if (!images.length) {
          images.push(...Array.from(clipboard?.items ?? [])
            .filter((item) => item.kind === "file" && item.type.toLocaleLowerCase().startsWith("image/"))
            .flatMap((item) => item.getAsFile() ?? []));
        }
        if (!images.length) return false;
        event.preventDefault();
        void (async () => {
          for (const file of images) {
            let attachment: DraftAttachmentSummary;
            try {
              attachment = await onAddInlineImage(file);
            } catch {
              return;
            }
            if (!attachment.contentId || !attachment.previewDataUrl) continue;
            const node = view.state.schema.nodes.nextmailImage?.create({
              src: `cid:${attachment.contentId}`,
              contentId: attachment.contentId,
              previewSrc: attachment.previewDataUrl,
              alt: file.name,
            });
            if (node) view.dispatch(view.state.tr.replaceSelectionWith(node));
          }
        })();
        return true;
      },
    },
    onUpdate: ({ editor: current }) => {
      const content = serializeEditor(current);
      setSourceHtml(content.html);
      onChange(content);
      onCompositionChange?.(compositionSelection(JSON.parse(content.editorJson) as JSONContent));
    },
  }, [extensions]);

  useImperativeHandle(ref, () => ({
    replaceTemplate: (definitionId, content) => replaceCompositionNode(
      editor,
      "nextmailTemplate",
      definitionId,
      content,
    ),
    replaceSignature: (definitionId, content) => replaceCompositionNode(
      editor,
      "nextmailSignature",
      definitionId,
      content,
    ),
  }), [editor]);

  useEffect(() => {
    editor?.setEditable(!disabled);
  }, [disabled, editor]);

  useEffect(() => {
    if (editor && !sourceHtml) setSourceHtml(serializeEditor(editor).html);
  }, [editor, sourceHtml]);

  if (!editor) return null;
  const richDisabled = disabled || sourceMode;
  const action = (label: string, active: boolean, onClick: () => void, icon: ReactNode) => (
    <Button
      type="button"
      size="icon"
      variant={active ? "secondary" : "ghost"}
      aria-label={label}
      title={label}
      disabled={richDisabled}
      onClick={onClick}
    >
      {icon}
    </Button>
  );
  const updateSource = (value: string) => {
    setSourceHtml(value);
    const document = hydrateInlineImagePreviews(documentFromHtml(value, extensions), previewMap);
    editor.commands.setContent(document, { emitUpdate: false });
    const persisted = stripTransientAttributes(document);
    onChange({
      editorJson: JSON.stringify(persisted),
      html: value,
      plainText: htmlToPlainText(value),
    });
    onCompositionChange?.(compositionSelection(persisted));
  };
  const toggleSourceMode = () => {
    if (!sourceMode) setSourceHtml(serializeEditor(editor).html);
    else editor.commands.setContent(
      hydrateInlineImagePreviews(documentFromHtml(sourceHtml, extensions), previewMap),
      { emitUpdate: false },
    );
    setSourceMode((value) => !value);
  };

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
          disabled={richDisabled}
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
          disabled={richDisabled}
          onValueChange={(value) => value === "default"
            ? editor.chain().focus().unsetFontSize().run()
            : editor.chain().focus().setFontSize(value).run()}
        />
        <ColorMenu
          label={t("composer.textColor")}
          icon={<Palette size={16} />}
          disabled={richDisabled}
          onSelect={(value) => value ? editor.chain().focus().setColor(value).run() : editor.chain().focus().unsetColor().run()}
        />
        <ColorMenu
          label={t("composer.backgroundColor")}
          icon={<Highlighter size={16} />}
          disabled={richDisabled}
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
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        {action(t("composer.undo"), false, () => editor.chain().focus().undo().run(), <Undo2 size={16} />)}
        {action(t("composer.redo"), false, () => editor.chain().focus().redo().run(), <Redo2 size={16} />)}
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        <Button
          type="button"
          size="icon"
          variant={sourceMode ? "secondary" : "ghost"}
          aria-label={t("composer.htmlSource")}
          title={t("composer.htmlSource")}
          disabled={disabled}
          onClick={toggleSourceMode}
        >
          <Code2 size={16} />
        </Button>
      </Inline>
      {sourceMode ? (
        <Page className="grid min-h-0 flex-1 grid-cols-2 divide-x divide-border overflow-hidden">
          <Page className="flex min-h-0 flex-col overflow-hidden">
            <Text className="shrink-0 bg-muted/35 px-3 py-2 text-xs font-semibold text-foreground">
              {t("composer.htmlSource")}
            </Text>
            <Page className="min-h-0 flex-1 overflow-hidden">
              <HtmlSourceEditor
                value={sourceHtml}
                ariaLabel={t("composer.htmlSource")}
                disabled={disabled}
                onChange={updateSource}
              />
            </Page>
          </Page>
          <Page className="flex min-h-0 flex-col overflow-hidden bg-white">
            <Text className="shrink-0 bg-muted/35 px-3 py-2 text-xs font-semibold text-foreground">
              {t("composer.htmlPreview")}
            </Text>
            <iframe
              className="min-h-0 flex-1 border-0 bg-white"
              title={t("composer.htmlPreview")}
              sandbox=""
              referrerPolicy="no-referrer"
              srcDoc={buildComposerPreviewDocument(sourceHtml, previewMap)}
            />
          </Page>
        </Page>
      ) : (
        <EditorContent
          editor={editor}
          className="nextmail-editor-scroll min-h-0 flex-1 overflow-x-auto overflow-y-scroll"
        />
      )}
    </Page>
  );
});

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

function parseDocument(value: string): JSONContent {
  try {
    return normalizeOriginalNodes(JSON.parse(value) as JSONContent);
  } catch {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
}

function documentFromHtml(value: string, extensions: Extensions): JSONContent {
  try {
    return normalizeOriginalNodes(generateJSON(value, extensions) as JSONContent);
  } catch {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
}

function normalizeOriginalNodes(node: JSONContent): JSONContent {
  if (node.type === "nextmailOriginalMessage") {
    const sourceHtml = typeof node.attrs?.sourceHtml === "string" ? node.attrs.sourceHtml : "";
    const sourcePlainText = typeof node.attrs?.sourcePlainText === "string"
      ? node.attrs.sourcePlainText
      : textFromJson(node);
    return {
      ...node,
      attrs: { ...node.attrs, sourceHtml, sourcePlainText },
      content: undefined,
    };
  }
  return {
    ...node,
    content: node.content?.map(normalizeOriginalNodes),
  };
}

function textFromJson(node: JSONContent): string {
  if (node.type === "text") return node.text ?? "";
  return (node.content ?? []).map(textFromJson).join("\n");
}

function serializeEditor(editor: Editor): DraftContent {
  const document = stripTransientAttributes(editor.getJSON());
  const html = materializeOriginalHtml(editor.getHTML(), document);
  return {
    editorJson: JSON.stringify(document),
    html,
    plainText: htmlToPlainText(html),
  };
}

function materializeOriginalHtml(value: string, document: JSONContent) {
  const parsed = new DOMParser().parseFromString(`<body>${value}</body>`, "text/html");
  const sources: string[] = [];
  visitNodes(document, (node) => {
    if (node.type === "nextmailOriginalMessage") {
      sources.push(typeof node.attrs?.sourceHtml === "string" ? node.attrs.sourceHtml : "");
    }
  });
  parsed.body.querySelectorAll("[data-nextmail-original-message]").forEach((element, index) => {
    element.innerHTML = sources[index] ?? "";
  });
  return parsed.body.innerHTML;
}

function stripTransientAttributes(node: JSONContent): JSONContent {
  const attrs = node.attrs ? { ...node.attrs } : undefined;
  if (attrs) delete attrs.previewSrc;
  return {
    ...node,
    attrs,
    content: node.content?.map(stripTransientAttributes),
  };
}

function hydrateInlineImagePreviews(
  node: JSONContent,
  previews: Record<string, string>,
): JSONContent {
  const attrs = node.attrs ? { ...node.attrs } : undefined;
  if (node.type === "nextmailImage" && attrs) {
    const contentId = typeof attrs.contentId === "string"
      ? attrs.contentId
      : typeof attrs.src === "string" && attrs.src.toLocaleLowerCase().startsWith("cid:")
        ? attrs.src.slice(4)
        : null;
    if (contentId) {
      attrs.contentId = contentId;
      attrs.previewSrc = previews[contentId.trim().replace(/^<|>$/g, "").toLocaleLowerCase()] ?? null;
    }
  }
  return {
    ...node,
    attrs,
    content: node.content?.map((child) => hydrateInlineImagePreviews(child, previews)),
  };
}

function compositionSelection(document: JSONContent): CompositionNodeSelection {
  const selection: CompositionNodeSelection = { templateId: null, signatureId: null };
  visitNodes(document, (node) => {
    if (node.type === "nextmailTemplate") {
      selection.templateId = typeof node.attrs?.definitionId === "string" ? node.attrs.definitionId : null;
    }
    if (node.type === "nextmailSignature") {
      selection.signatureId = typeof node.attrs?.definitionId === "string" ? node.attrs.definitionId : null;
    }
  });
  return selection;
}

function visitNodes(node: JSONContent, visitor: (node: JSONContent) => void) {
  visitor(node);
  node.content?.forEach((child) => visitNodes(child, visitor));
}

function replaceCompositionNode(
  editor: Editor | null,
  nodeType: "nextmailTemplate" | "nextmailSignature",
  definitionId: string | null,
  content?: DraftContent,
) {
  if (!editor) return false;
  const document = editor.getJSON();
  const target = findNode(document, nodeType);
  if (!definitionId) {
    if (!target) return true;
    return editor.chain().deleteRange({ from: target.from, to: target.to }).run();
  }
  const children = parseDocument(content?.editorJson ?? "").content;
  const node = {
    type: nodeType,
    attrs: { definitionId },
    content: Array.isArray(children) && children.length ? children : [{ type: "paragraph" }],
  };
  if (target) {
    return editor.chain().insertContentAt(target, node).run();
  }
  if (nodeType === "nextmailTemplate") {
    const reply = findNode(document, "nextmailReply");
    if (reply) {
      return insertNodeAt(editor, reply.from + 1, node);
    }
    return editor.chain().insertContentAt(0, node).run();
  }
  const original = findNode(document, "nextmailOriginalMessage");
  if (original) {
    return insertNodeAt(editor, original.from, node);
  }
  return editor.chain().insertContentAt(editor.state.doc.content.size, [
    { type: "paragraph" },
    node,
  ]).run();
}

function insertNodeAt(editor: Editor, position: number, node: JSONContent) {
  try {
    const transaction = editor.state.tr.insert(position, editor.schema.nodeFromJSON(node));
    editor.view.dispatch(transaction);
    return true;
  } catch {
    return false;
  }
}

function findNode(
  document: JSONContent,
  nodeType: string,
) {
  return findNodeInChildren(document.content ?? [], nodeType, 0);
}

function findNodeInChildren(
  children: JSONContent[],
  nodeType: string,
  start: number,
): { from: number; to: number } | null {
  let position = start;
  for (const node of children) {
    const size = nodeSize(node);
    if (node.type === nodeType) return { from: position, to: position + size };
    const nested = findNodeInChildren(node.content ?? [], nodeType, position + 1);
    if (nested) return nested;
    position += size;
  }
  return null;
}

function nodeSize(node: JSONContent): number {
  if (node.type === "text") return node.text?.length ?? 0;
  const contentSize: number = (node.content ?? []).reduce(
    (sum: number, child: JSONContent) => sum + nodeSize(child),
    0,
  );
  return 2 + contentSize;
}
