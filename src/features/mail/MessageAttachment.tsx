import {
  ExternalLink,
  File,
  FileArchive,
  FileAudio,
  FileCode2,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  FolderOpen,
  ListRestart,
  Presentation,
  Save,
  type LucideIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AttachmentSummary } from "@/app/types";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Inline, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { InlineText, Text } from "@/components/ui/typography";
import { cn } from "@/lib/utils";

export function MessageAttachment({
  attachment,
  opening,
  saving,
  revealing,
  onOpen,
  onSaveAs,
  onReveal,
}: {
  attachment: AttachmentSummary;
  opening: boolean;
  saving: boolean;
  revealing: boolean;
  onOpen: () => void;
  onSaveAs: () => void;
  onReveal: () => void;
}) {
  const { t } = useTranslation();
  const available = attachment.availability === "available";
  const busy = opening || saving || revealing;
  const fileVisual = attachmentFileVisual(attachment);
  const FileIcon = fileVisual.icon;
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <button
          type="button"
          className="h-12 w-64 max-w-full min-w-0 rounded-md border border-border/70 bg-muted/35 px-2.5 text-left text-foreground outline-none transition-colors hover:border-primary/25 hover:bg-muted/60 focus-visible:shadow-[inset_0_0_0_1px_var(--primary)] disabled:opacity-70"
          title={attachment.fileName}
          aria-label={t("mail.openAttachment", { name: attachment.fileName })}
          disabled={busy}
          onClick={onOpen}
        >
          <Inline className="min-w-0 gap-2.5">
            <span className={cn("grid size-8 shrink-0 place-items-center rounded-sm", fileVisual.className)}>
              {busy && !available ? <Spinner size={16} /> : <FileIcon size={17} aria-hidden="true" />}
            </span>
            <Stack className="min-w-0 gap-0">
              <InlineText className="block min-w-0 truncate text-[13px] font-medium text-inherit">{attachment.fileName}</InlineText>
              <Text className="text-[11px] leading-4">{formatBytes(attachment.size)}</Text>
            </Stack>
          </Inline>
        </button>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem disabled={busy} onSelect={onOpen}>
          <ExternalLink size={14} />{t("mail.open")}
        </ContextMenuItem>
        <ContextMenuItem disabled>
          <ListRestart size={14} />{t("mail.openWith")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem disabled={busy} onSelect={onReveal}>
          <FolderOpen size={14} />{t("mail.showInFolder")}
        </ContextMenuItem>
        <ContextMenuItem disabled={busy} onSelect={onSaveAs}>
          <Save size={14} />{t("mail.saveAs")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

export function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

interface AttachmentFileVisual {
  icon: LucideIcon;
  className: string;
}

function attachmentFileVisual(attachment: AttachmentSummary): AttachmentFileVisual {
  const contentType = attachment.contentType.toLocaleLowerCase();
  const extension = attachment.fileName.split(".").pop()?.toLocaleLowerCase() ?? "";
  if (contentType === "application/pdf" || extension === "pdf") {
    return { icon: FileText, className: "bg-red-500/12 text-red-600 dark:text-red-400" };
  }
  if (contentType.includes("word") || ["doc", "docx", "odt", "rtf"].includes(extension)) {
    return { icon: FileText, className: "bg-blue-500/12 text-blue-600 dark:text-blue-400" };
  }
  if (contentType.includes("spreadsheet") || contentType.includes("excel") || ["xls", "xlsx", "ods", "csv"].includes(extension)) {
    return { icon: FileSpreadsheet, className: "bg-emerald-500/12 text-emerald-600 dark:text-emerald-400" };
  }
  if (contentType.includes("presentation") || contentType.includes("powerpoint") || ["ppt", "pptx", "odp"].includes(extension)) {
    return { icon: Presentation, className: "bg-orange-500/12 text-orange-600 dark:text-orange-400" };
  }
  if (contentType.startsWith("image/")) {
    return { icon: FileImage, className: "bg-violet-500/12 text-violet-600 dark:text-violet-400" };
  }
  if (contentType.startsWith("audio/")) {
    return { icon: FileAudio, className: "bg-pink-500/12 text-pink-600 dark:text-pink-400" };
  }
  if (contentType.startsWith("video/")) {
    return { icon: FileVideo, className: "bg-indigo-500/12 text-indigo-600 dark:text-indigo-400" };
  }
  if (contentType.includes("zip")
    || contentType.includes("compressed")
    || ["zip", "rar", "7z", "tar", "gz", "bz2", "xz"].includes(extension)) {
    return { icon: FileArchive, className: "bg-amber-500/12 text-amber-700 dark:text-amber-400" };
  }
  if (contentType.startsWith("text/")
    || ["html", "htm", "xml", "json", "js", "ts", "tsx", "jsx", "css", "rs", "toml", "yaml", "yml"].includes(extension)) {
    return { icon: FileCode2, className: "bg-slate-500/12 text-slate-600 dark:text-slate-400" };
  }
  return { icon: File, className: "bg-muted-foreground/10 text-muted-foreground" };
}
