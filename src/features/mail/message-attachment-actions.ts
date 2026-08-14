import type { AttachmentSummary } from "@/app/types";

export async function activateMessageAttachment(
  attachment: AttachmentSummary,
  actions: {
    download: (attachmentId: string) => Promise<unknown>;
    open: (attachmentId: string) => Promise<unknown>;
  },
) {
  if (attachment.availability !== "available") {
    await actions.download(attachment.id);
  }
  await actions.open(attachment.id);
}
