import { EditorContent, useEditor } from "@tiptap/react";
import { generateJSON } from "@tiptap/core";
import type { Editor, Extensions, JSONContent } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import { TextStyleKit, TextStyle } from "@tiptap/extension-text-style";
import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";

import { useTranslation } from "react-i18next";

import type { DraftAttachmentSummary, DraftContent } from "@/app/types";
import { Button } from "@/components/ui/button";
import { TextField } from "@/components/ui/input";
import { Form, Page } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";

import { Text } from "@/components/ui/typography";
import { cn } from "@/lib/utils";
import {
  createNextMailOriginalMessage,
  NextMailReply,
  NextMailSignature,
  NextMailSignatureDivider,
  NextMailTemplate,
} from "./composition-nodes";
import {
  buildComposerPreviewDocument,
  COMPOSER_CONTENT_STYLE,
  ensureComposerContentHtml,
  htmlToPlainText,
  inlineImagePreviews,
  stripComposerContentEnvelope,
} from "./composer-html";
import { HtmlSourceEditor } from "./HtmlSourceEditor";
import {
  EmailBlock,
  EmailFont,
  EmailFormattingAttributes,
  EmailInlineBlock,
  EmailSpan,
  EmailStylesheet,
  EmailTable,
  EmailTableCell,
  EmailTableHeader,
  EmailTableRow,
  NextMailImage,
} from "./email-content-nodes";

import { EditorToolbar } from "./EditorToolbar";
import { ComposerTextAttributes } from "./editor-formatting";
import { ComposerEditing } from "./editor-keymap";

const BASE_COMPOSER_EXTENSIONS: Extensions = [
  StarterKit.configure({
    underline: false,
    link: {
      openOnClick: false,
      autolink: false,
      linkOnPaste: false,
      enableClickSelection: true,
      HTMLAttributes: {
        target: "_blank",
        rel: "noopener noreferrer",
      },
    },
    trailingNode: false,
  }),
  Underline,
  TextStyleKit.configure({ textStyle: false }),
  TextStyle.extend({ priority: 90 }),
  ComposerTextAttributes,
  ComposerEditing,
  EmailFormattingAttributes,
  EmailSpan,
  EmailStylesheet,
  EmailInlineBlock,
  EmailBlock,
  EmailFont,
  EmailTable,
  EmailTableRow,
  EmailTableCell,
  EmailTableHeader,
  NextMailImage,
  NextMailTemplate,
  NextMailSignatureDivider,
  NextMailSignature,
  NextMailReply,
];

interface RichTextEditorProps {
  initialJson: string;
  initialHtml?: string;
  disabled?: boolean;
  ariaLabel?: string;
  className?: string;
  onChange: (content: DraftContent) => void;
  onCompositionChange?: (selection: CompositionNodeSelection) => void;
  inlineImages?: DraftAttachmentSummary[];
  includePortableStyles?: boolean;
  onAddInlineImage?: (file: File) => Promise<RichTextInlineImage>;
  onSanitizeHtml?: (html: string) => Promise<string>;
}

export interface RichTextInlineImage {
  fileName: string;
  contentType: string;
  size: number;
  contentId: string | null;
  previewDataUrl: string | null;
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
    initialHtml,
    disabled,
    ariaLabel,
    className,
    onChange,
    onCompositionChange,
    inlineImages = [],
    includePortableStyles = true,
    onAddInlineImage,
    onSanitizeHtml,
  },
  ref,
) {
  const { t } = useTranslation();
  const inlineImagesRef = useRef(inlineImages);
  inlineImagesRef.current = inlineImages;
  const addInlineImageRef = useRef(onAddInlineImage);
  addInlineImageRef.current = onAddInlineImage;
  const sanitizeHtmlRef = useRef(onSanitizeHtml);
  sanitizeHtmlRef.current = onSanitizeHtml;
  const disabledRef = useRef(disabled);
  disabledRef.current = disabled;
  const editorInstanceRef = useRef<Editor | null>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const previewMap = useMemo(() => inlineImagePreviews(inlineImages), [inlineImages]);
  const extensions = useMemo<Extensions>(() => [
    ...BASE_COMPOSER_EXTENSIONS,
    createNextMailOriginalMessage(
      () => inlineImagePreviews(inlineImagesRef.current),
      t("composer.htmlPreview"),
    ),
  ], [t]);
  const initialContent = useMemo(
    () => hydrateInlineImagePreviews(initialDocument(initialJson, initialHtml, extensions), previewMap),
    [initialJson, initialHtml, extensions, previewMap],
  );
  const [sourceMode, setSourceMode] = useState(false);
  const [sourceHtml, setSourceHtml] = useState(() => includePortableStyles
    ? ensureComposerContentHtml(initialHtml ?? "")
    : initialHtml ?? "");
  const sourceInitializedRef = useRef(initialHtml !== undefined);
  const [linkEditorOpen, setLinkEditorOpen] = useState(false);
  const [linkHref, setLinkHref] = useState("");
  const [linkError, setLinkError] = useState("");
  const editor = useEditor({
    extensions,
    content: initialContent,
    editable: !disabled,
    editorProps: {
      attributes: {
        class: "nextmail-editor-content",
        "data-nextmail-composer-body": "",
        "aria-label": ariaLabel ?? t("composer.body"),
      },
      handleKeyDown: (view, event) => {
        if (disabledRef.current || event.key !== "Tab") return false;
        event.preventDefault();
        view.dispatch(view.state.tr.insertText("\u00a0".repeat(4)));
        return true;
      },
      handlePaste: (_view, event) => {
        const clipboard = event.clipboardData;
        const html = clipboard?.getData("text/html") ?? "";
        const plainText = clipboard?.getData("text/plain") ?? "";
        const images = clipboardImageFiles(clipboard);
        if (disabledRef.current || (!html && !images.length)) return false;
        event.preventDefault();
        const currentEditor = editorInstanceRef.current;
        if (currentEditor) void insertClipboardContent(
          currentEditor,
          html,
          plainText,
          images,
          addInlineImageRef.current,
          sanitizeHtmlRef.current,
        );
        return true;
      },
    },
    onUpdate: ({ editor: current }) => {
      const content = serializeEditor(current, includePortableStyles);
      setSourceHtml(content.html);
      onChange(content);
      onCompositionChange?.(compositionSelection(JSON.parse(content.editorJson) as JSONContent));
    },
  }, [extensions]);
  editorInstanceRef.current = editor;

  useImperativeHandle(ref, () => ({
    replaceTemplate: (definitionId, content) => {
      return replaceCompositionNode(
        editor,
        "nextmailTemplate",
        definitionId,
        content,
      );
    },
    replaceSignature: (definitionId, content) => {
      return replaceCompositionNode(
        editor,
        "nextmailSignature",
        definitionId,
        content,
      );
    },
  }), [editor]);

  useEffect(() => {
    editor?.setEditable(!disabled, false);
  }, [disabled, editor]);

  useEffect(() => {
    if (!editor || sourceInitializedRef.current) return;
    setSourceHtml(serializeEditor(editor, includePortableStyles).html);
    sourceInitializedRef.current = true;
  }, [editor, includePortableStyles]);

  if (!editor) return null;
  const richDisabled = disabled || sourceMode;
  const commitRichEdit = (callback: () => void) => {
    callback();
  };
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
    if (sourceMode) editor.commands.setContent(
      hydrateInlineImagePreviews(documentFromHtml(sourceHtml, extensions), previewMap),
      { emitUpdate: false },
    );
    setLinkEditorOpen(false);
    setSourceMode((value) => !value);
  };
  const openLinkEditor = () => {
    const href = editor.getAttributes("link").href;
    setLinkHref(typeof href === "string" ? href : "");
    setLinkError("");
    setLinkEditorOpen(true);
  };
  const applyLink = () => {
    const normalized = normalizeComposerLinkTarget(linkHref);
    if (!normalized) {
      setLinkError(t("composer.invalidLink"));
      return;
    }
    const chain = editor.chain().focus();
    if (editor.isActive("link")) chain.extendMarkRange("link");
    if (editor.state.selection.empty && !editor.isActive("link")) {
      chain.insertContent({
        type: "text",
        text: normalized,
        marks: [{ type: "link", attrs: { href: normalized } }],
      }).run();
    } else {
      chain.setLink({ href: normalized }).run();
    }
    setLinkEditorOpen(false);
    setLinkError("");
  };
  const removeLink = () => {
    editor.chain().focus().extendMarkRange("link").unsetLink().run();
    setLinkEditorOpen(false);
    setLinkError("");
  };

  return (
    <Page className={cn("flex min-h-0 flex-1 flex-col bg-card", className)}>
      <style data-nextmail-local-composer-style="">{COMPOSER_CONTENT_STYLE}</style>
      <EditorToolbar editor={editor} disabled={disabled} sourceMode={sourceMode} onEdit={commitRichEdit} onLink={openLinkEditor} onRemoveLink={removeLink} onSource={toggleSourceMode} onImage={onAddInlineImage ? () => imageInputRef.current?.click() : undefined} />
      {onAddInlineImage ? <input ref={imageInputRef} className="sr-only" type="file" accept="image/png,image/jpeg,image/gif,image/webp,image/bmp" multiple aria-label={t("composer.insertImage")} disabled={richDisabled} onChange={event => {
        const files = Array.from(event.currentTarget.files ?? []);
        event.currentTarget.value = "";
        if (files.length) void insertCachedImages(editor, files, onAddInlineImage);
      }} /> : null}
      {linkEditorOpen && !sourceMode ? (
        <Form
          className="flex shrink-0 items-end gap-2 border-t border-border bg-card px-3 py-2"
          onSubmit={(event) => {
            event.preventDefault();
            applyLink();
          }}
        >
          <TextField
            className="max-w-xl"
            label={t("composer.linkUrl")}
            placeholder={t("composer.linkUrlPlaceholder")}
            value={linkHref}
            error={linkError}
            autoFocus
            disabled={disabled}
            onChange={(event) => {
              setLinkHref(event.target.value);
              setLinkError("");
            }}
          />
          {editor.isActive("link") ? (
            <Button type="button" variant="ghost" disabled={disabled} onClick={removeLink}>
              {t("composer.removeLink")}
            </Button>
          ) : null}
          <Button
            type="button"
            variant="ghost"
            disabled={disabled}
            onClick={() => {
              setLinkEditorOpen(false);
              setLinkError("");
            }}
          >
            {t("common.cancel")}
          </Button>
          <Button type="submit" disabled={disabled}>{t("composer.applyLink")}</Button>
        </Form>
      ) : null}
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
              srcDoc={buildComposerPreviewDocument(sourceHtml, previewMap, true)}
            />
          </Page>
        </Page>
      ) : (
        <OverlayScrollArea
          className="min-h-0 flex-1"
          viewportClassName="flex flex-col"
          contentClassName="flex-1 shrink-0"
        >
          <EditorContent
            editor={editor}
            className="nextmail-editor-surface flex min-w-0 flex-1 flex-col overflow-x-auto"
          />
        </OverlayScrollArea>
      )}
    </Page>
  );
});

function clipboardImageFiles(clipboard: DataTransfer | null) {
  const files = Array.from(clipboard?.files ?? [])
    .filter((file) => file.type.toLocaleLowerCase().startsWith("image/"));
  if (files.length) return files;
  return Array.from(clipboard?.items ?? [])
    .filter((item) => item.kind === "file" && item.type.toLocaleLowerCase().startsWith("image/"))
    .flatMap((item) => item.getAsFile() ?? []);
}

async function insertClipboardContent(
  editor: Editor,
  html: string,
  plainText: string,
  files: File[],
  addInlineImage?: (file: File) => Promise<RichTextInlineImage>,
  sanitizeHtml?: (html: string) => Promise<string>,
) {
  let remainingFiles = files;
  if (html && sanitizeHtml) {
    try {
      const sanitized = await sanitizeHtml(html);
      const prepared = addInlineImage
        ? await cacheInlineImagesInHtml(sanitized, files, addInlineImage)
        : { html: sanitized, remainingFiles: files };
      remainingFiles = prepared.remainingFiles;
      editor.chain().focus().insertContent(prepared.html).run();
    } catch {
      if (plainText) editor.chain().focus().insertContent(plainText).run();
    }
  } else if (plainText) {
    editor.chain().focus().insertContent(plainText).run();
  }
  if (addInlineImage && remainingFiles.length) {
    await insertCachedImages(editor, remainingFiles, addInlineImage);
  }
}

async function cacheInlineImagesInHtml(
  html: string,
  files: File[],
  addInlineImage: (file: File) => Promise<RichTextInlineImage>,
) {
  const matches = Array.from(html.matchAll(/(\s+src\s*=\s*)(["'])([^"']*)\2/gi));
  const remainingFiles = [...files];
  let output = "";
  let cursor = 0;
  for (const match of matches) {
    const start = match.index ?? cursor;
    output += html.slice(cursor, start);
    const clipboardFile = remainingFiles.shift();
    const file = clipboardFile ?? dataImageFile(match[3], start);
    let replacement = match[0];
    if (file) {
      try {
        const attachment = await addInlineImage(file);
        const source = attachment.contentId
          ? `cid:${attachment.contentId}`
          : attachment.previewDataUrl;
        if (source) {
          replacement = `${match[1]}${match[2]}${escapeHtmlAttribute(source)}${match[2]}`;
          if (attachment.contentId && attachment.previewDataUrl) {
            replacement += ` data-nextmail-preview-src="${escapeHtmlAttribute(attachment.previewDataUrl)}"`;
          }
        }
      } catch {
        // Keep the sanitized source when one clipboard image cannot be cached.
      }
    }
    output += replacement;
    cursor = start + match[0].length;
  }
  output += html.slice(cursor);
  return { html: output, remainingFiles };
}

async function insertCachedImages(
  editor: Editor,
  files: File[],
  addInlineImage: (file: File) => Promise<RichTextInlineImage>,
) {
  for (const file of files) {
    let attachment: RichTextInlineImage;
    try {
      attachment = await addInlineImage(file);
    } catch {
      continue;
    }
    const source = attachment.contentId
      ? `cid:${attachment.contentId}`
      : attachment.previewDataUrl;
    if (!source) continue;
    editor.chain().focus().insertContent({
      type: "nextmailImage",
      attrs: {
        src: source,
        contentId: attachment.contentId,
        previewSrc: attachment.previewDataUrl,
        alt: attachment.fileName || file.name,
      },
    }).run();
  }
}

function dataImageFile(value: string, index: number) {
  const match = /^data:(image\/(?:png|jpeg|gif|webp|bmp));base64,([a-z0-9+/=\s]+)$/i.exec(value.trim());
  if (!match) return null;
  try {
    const binary = window.atob(match[2].replace(/\s/g, ""));
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const extension = match[1].toLocaleLowerCase() === "image/jpeg"
      ? "jpg"
      : match[1].slice("image/".length).toLocaleLowerCase();
    return new File([bytes], `pasted-image-${index}.${extension}`, { type: match[1].toLocaleLowerCase() });
  } catch {
    return null;
  }
}

function escapeHtmlAttribute(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function normalizeComposerLinkTarget(candidate: string) {
  const trimmed = candidate.trim();
  if (
    !trimmed
    || new TextEncoder().encode(trimmed).length > 16 * 1024
    || trimmed.includes("\\")
    || containsConfusingCharacters(trimmed)
    || containsPercentEncodedConfusingCharacters(trimmed)
  ) {
    return null;
  }
  const input = trimmed.startsWith("//") ? `https:${trimmed}` : trimmed;
  let url: URL;
  try {
    url = new URL(input);
  } catch {
    return null;
  }
  if (url.protocol === "http:" || url.protocol === "https:") {
    if (!url.hostname || url.username || url.password) return null;
  } else if (url.protocol === "mailto:") {
    if (!url.pathname.trim() || url.host) return null;
  } else {
    return null;
  }
  const target = url.toString();
  return new TextEncoder().encode(target).length <= 16 * 1024
    && !containsConfusingCharacters(target)
    ? target
    : null;
}

function containsConfusingCharacters(value: string) {
  return Array.from(value).some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint <= 0x1f
      || (codePoint >= 0x7f && codePoint <= 0x9f)
      || codePoint === 0x061c
      || codePoint === 0x200e
      || codePoint === 0x200f
      || (codePoint >= 0x202a && codePoint <= 0x202e)
      || (codePoint >= 0x2066 && codePoint <= 0x2069);
  });
}

function containsPercentEncodedConfusingCharacters(value: string) {
  const bytes = Array.from(new TextEncoder().encode(value));
  const decoded: number[] = [];
  for (let index = 0; index < bytes.length; index += 1) {
    if (
      bytes[index] === 0x25
      && index + 2 < bytes.length
      && isHexByte(bytes[index + 1])
      && isHexByte(bytes[index + 2])
    ) {
      decoded.push(Number.parseInt(String.fromCharCode(bytes[index + 1], bytes[index + 2]), 16));
      index += 2;
    } else {
      decoded.push(bytes[index]);
    }
  }
  if (decoded.some((byte) => byte <= 0x1f || byte === 0x7f)) return true;
  try {
    return containsConfusingCharacters(
      new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(decoded)),
    );
  } catch {
    return false;
  }
}

function isHexByte(value: number) {
  return (value >= 0x30 && value <= 0x39)
    || (value >= 0x41 && value <= 0x46)
    || (value >= 0x61 && value <= 0x66);
}

function initialDocument(json: string, html: string | undefined, extensions: Extensions): JSONContent {
  const document = parseDocument(json);
  // Rust imports complete, sanitized MIME as scoped HTML. Hydrate it only when
  // there is no saved editor tree; never replace an existing user's edits.
  return document.type === "doc" && document.content?.length === 0 && html !== undefined
    ? documentFromHtml(html, extensions)
    : document;
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
    return normalizeOriginalNodes(generateJSON(
      stripComposerContentEnvelope(value),
      extensions,
    ) as JSONContent);
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

function serializeEditor(editor: Editor, includePortableStyles: boolean): DraftContent {
  const document = stripTransientAttributes(editor.getJSON());
  const materialized = materializeOriginalHtml(editor.getHTML(), document);
  const html = includePortableStyles ? ensureComposerContentHtml(materialized) : materialized;
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
  const target = findEditorNode(editor, nodeType);
  if (!definitionId) {
    if (!target) return true;
    if (nodeType === "nextmailSignature") {
      const divider = findEditorNode(editor, "nextmailSignatureDivider");
      if (divider?.to === target.from) {
        return editor.chain().deleteRange({ from: divider.from, to: target.to }).run();
      }
    }
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
    const reply = findEditorNode(editor, "nextmailReply");
    if (reply) {
      return insertNodeAt(editor, reply.from + 1, node);
    }
    return editor.chain().insertContentAt(0, node).run();
  }
  const original = findEditorNode(editor, "nextmailOriginalMessage");
  if (original) {
    return editor.chain().insertContentAt(original.from, [
      { type: "nextmailSignatureDivider" },
      node,
      { type: "paragraph" },
    ]).run();
  }
  return editor.chain().insertContentAt(editor.state.doc.content.size, [
    { type: "paragraph" },
    { type: "nextmailSignatureDivider" },
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

function findEditorNode(
  editor: Editor,
  nodeType: string,
): { from: number; to: number } | null {
  let result: { from: number; to: number } | null = null;
  editor.state.doc.descendants((node, position) => {
    if (result) return false;
    if (node.type.name !== nodeType) return true;
    result = { from: position, to: position + node.nodeSize };
    return false;
  });
  return result;
}
