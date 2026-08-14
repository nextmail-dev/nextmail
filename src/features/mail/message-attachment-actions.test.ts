import { describe, expect, it, vi } from "vitest";

import { activateMessageAttachment } from "./message-attachment-actions";

const missingAttachment = {
  id: "attachment",
  fileName: "report.pdf",
  contentType: "application/pdf",
  size: 10,
  availability: "missing" as const,
};

describe("activateMessageAttachment", () => {
  it("downloads before opening unavailable attachments", async () => {
    const order: string[] = [];
    await activateMessageAttachment(missingAttachment, {
      download: vi.fn(async () => { order.push("download"); }),
      open: vi.fn(async () => { order.push("open"); }),
    });
    expect(order).toEqual(["download", "open"]);
  });

  it("opens an available attachment without downloading again", async () => {
    const download = vi.fn().mockResolvedValue(undefined);
    const open = vi.fn().mockResolvedValue(undefined);

    await activateMessageAttachment({ ...missingAttachment, availability: "available" }, { download, open });

    expect(download).not.toHaveBeenCalled();
    expect(open).toHaveBeenCalledWith("attachment");
  });
});
