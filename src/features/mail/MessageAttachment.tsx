import { Download, ExternalLink, Save } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AttachmentSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Inline } from "@/components/ui/layout";
import { InlineText } from "@/components/ui/typography";

export function MessageAttachment({
  attachment,
  opening,
  saving,
  onOpen,
  onSaveAs,
}: {
  attachment: AttachmentSummary;
  opening: boolean;
  saving: boolean;
  onOpen: () => void;
  onSaveAs: () => void;
}) {
  const { t } = useTranslation();
  const available = attachment.availability === "available";
  return (
    <Inline className="max-w-full gap-0.5 rounded-md bg-secondary p-0.5">
      <Button
        variant="ghost"
        size="sm"
        className="min-w-0 max-w-80 justify-start px-2.5 text-foreground hover:bg-card"
        loading={opening}
        title={attachment.fileName}
        aria-label={available ? t("mail.openAttachment", { name: attachment.fileName }) : t("mail.downloadAttachment", { name: attachment.fileName })}
        onClick={onOpen}
      >
        {available ? <ExternalLink size={14} /> : <Download size={14} />}
        <InlineText className="min-w-0 truncate text-inherit">{attachment.fileName}</InlineText>
        <InlineText className="shrink-0 text-[length:var(--ui-font-caption)] text-muted-foreground">{formatBytes(attachment.size)}</InlineText>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="size-8 text-muted-foreground hover:bg-card hover:text-foreground"
        loading={saving}
        aria-label={t("mail.saveAttachmentAs", { name: attachment.fileName })}
        title={t("mail.saveAs")}
        onClick={onSaveAs}
      >
        <Save size={14} />
      </Button>
    </Inline>
  );
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}
