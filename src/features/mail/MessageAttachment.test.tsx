import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import { MessageAttachment } from "./MessageAttachment";

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("MessageAttachment", () => {
  it("keeps a downloaded attachment openable and exposes save as", () => {
    const onOpen = vi.fn();
    const onSaveAs = vi.fn();
    render(
      <MessageAttachment
        attachment={{ id: "a", fileName: "report.pdf", contentType: "application/pdf", size: 2048, availability: "available" }}
        opening={false}
        saving={false}
        onOpen={onOpen}
        onSaveAs={onSaveAs}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open report.pdf" }));
    fireEvent.click(screen.getByRole("button", { name: "Save report.pdf as" }));
    expect(onOpen).toHaveBeenCalledOnce();
    expect(onSaveAs).toHaveBeenCalledOnce();
  });

  it("labels unavailable content as a download action", () => {
    render(
      <MessageAttachment
        attachment={{ id: "a", fileName: "archive.zip", contentType: "application/zip", size: 512, availability: "missing" }}
        opening={false}
        saving={false}
        onOpen={vi.fn()}
        onSaveAs={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Download archive.zip" })).toBeEnabled();
  });
});
