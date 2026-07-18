import type { AttachmentSummary } from "@/app/types";

export async function activateMessageAttachment(
  attachment: AttachmentSummary,
  autoOpenAfterDownload: boolean,
  actions: {
    download: (attachmentId: string) => Promise<unknown>;
    open: (attachmentId: string) => Promise<unknown>;
  },
) {
  if (attachment.availability !== "available") {
    await actions.download(attachment.id);
    if (!autoOpenAfterDownload) return;
  }
  await actions.open(attachment.id);
}
