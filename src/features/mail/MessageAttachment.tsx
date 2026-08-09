import {
  Download,
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
  Presentation,
  Save,
  type LucideIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AttachmentSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Inline, Stack } from "@/components/ui/layout";
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
  const fileVisual = attachmentFileVisual(attachment);
  const FileIcon = fileVisual.icon;
  return (
    <div className="group/attachment relative h-[72px] w-80 max-w-full overflow-hidden rounded-lg border border-border/70 bg-muted/45 transition-colors hover:border-primary/25 hover:bg-muted/70 focus-within:border-primary/35">
      <Button
        variant="ghost"
        className="size-full min-w-0 justify-start rounded-lg bg-transparent p-3 pr-[116px] text-left text-foreground hover:bg-transparent"
        loading={opening}
        title={attachment.fileName}
        aria-label={t("mail.attachmentPrimaryAction", { name: attachment.fileName })}
        onClick={onOpen}
      >
        <Inline className="min-w-0 gap-3">
          <span className={cn("grid size-10 shrink-0 place-items-center rounded-md", fileVisual.className)}>
            <FileIcon size={21} aria-hidden="true" />
          </span>
          <Stack className="min-w-0" gap="xs">
            <InlineText className="block min-w-0 truncate text-sm font-medium text-inherit">{attachment.fileName}</InlineText>
            <Text className="text-[length:var(--ui-font-caption)]">{formatBytes(attachment.size)}</Text>
          </Stack>
        </Inline>
      </Button>
      <Inline className="absolute right-2 top-1/2 -translate-y-1/2 gap-0.5 opacity-0 transition-opacity group-hover/attachment:opacity-100 group-focus-within/attachment:opacity-100">
        <Button
          variant="ghost"
          size="icon"
          className="size-8 bg-card/90 text-muted-foreground shadow-sm hover:bg-card hover:text-foreground"
          loading={opening}
          aria-label={available ? t("mail.openAttachment", { name: attachment.fileName }) : t("mail.downloadAttachment", { name: attachment.fileName })}
          title={available ? t("mail.openAttachment", { name: attachment.fileName }) : t("mail.downloadAttachment", { name: attachment.fileName })}
          onClick={onOpen}
        >
          {available ? <ExternalLink size={15} /> : <Download size={15} />}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-8 bg-card/90 text-muted-foreground shadow-sm hover:bg-card hover:text-foreground"
          loading={saving}
          aria-label={t("mail.saveAttachmentAs", { name: attachment.fileName })}
          title={t("mail.saveAs")}
          onClick={onSaveAs}
        >
          <Save size={15} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-8 bg-card/90 text-muted-foreground shadow-sm hover:bg-card hover:text-foreground"
          loading={revealing}
          aria-label={t("mail.revealAttachment", { name: attachment.fileName })}
          title={t("mail.showInFolder")}
          onClick={onReveal}
        >
          <FolderOpen size={15} />
        </Button>
      </Inline>
    </div>
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
