import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import type { AttachmentSummary } from "@/app/types";
import { MessageAttachment } from "./MessageAttachment";

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

describe("MessageAttachment", () => {
  it("keeps single-click open and exposes secondary actions in the context menu", async () => {
    const onOpen = vi.fn();
    const onSaveAs = vi.fn();
    const onReveal = vi.fn();
    render(
      <MessageAttachment
        attachment={{ id: "a", fileName: "report.pdf", contentType: "application/pdf", size: 2048, availability: "available" }}
        downloading={false}
        opening={false}
        saving={false}
        revealing={false}
        onOpen={onOpen}
        onSaveAs={onSaveAs}
        onReveal={onReveal}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open report.pdf" }));
    fireEvent.contextMenu(screen.getByRole("button", { name: "Open report.pdf" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Save as" }));
    fireEvent.contextMenu(screen.getByRole("button", { name: "Open report.pdf" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show in folder" }));
    expect(onOpen).toHaveBeenCalledOnce();
    expect(onSaveAs).toHaveBeenCalledOnce();
    expect(onReveal).toHaveBeenCalledOnce();
  });

  it("shows a spinner while an unavailable attachment is downloading", () => {
    const { container } = render(
      <MessageAttachment
        attachment={{ id: "a", fileName: "archive.zip", contentType: "application/zip", size: 512, availability: "missing" }}
        downloading={false}
        opening
        saving={false}
        revealing={false}
        onOpen={vi.fn()}
        onSaveAs={vi.fn()}
        onReveal={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Open archive.zip" })).toBeDisabled();
    expect(container.querySelector(".animate-spin")).not.toBeNull();
  });

  it("does not spin while the save-as dialog is open and only spins once the download starts", () => {
    const attachment: AttachmentSummary = { id: "a", fileName: "archive.zip", contentType: "application/zip", size: 512, availability: "missing" };
    const { container, rerender } = render(
      <MessageAttachment
        attachment={attachment}
        downloading={false}
        opening={false}
        saving
        revealing={false}
        onOpen={vi.fn()}
        onSaveAs={vi.fn()}
        onReveal={vi.fn()}
      />,
    );
    expect(container.querySelector(".animate-spin")).toBeNull();

    rerender(
      <MessageAttachment
        attachment={attachment}
        downloading
        opening={false}
        saving
        revealing={false}
        onOpen={vi.fn()}
        onSaveAs={vi.fn()}
        onReveal={vi.fn()}
      />,
    );
    expect(container.querySelector(".animate-spin")).not.toBeNull();
  });

  it("marks undownloaded attachments with a cloud badge next to the size", () => {
    const { container, rerender } = render(
      <MessageAttachment
        attachment={{ id: "a", fileName: "archive.zip", contentType: "application/zip", size: 512, availability: "missing" }}
        downloading={false}
        opening={false}
        saving={false}
        revealing={false}
        onOpen={vi.fn()}
        onSaveAs={vi.fn()}
        onReveal={vi.fn()}
      />,
    );
    const badge = container.querySelector("svg.lucide-cloud");
    expect(badge).not.toBeNull();
    expect(badge?.getAttribute("aria-label")).toBe("Not downloaded yet");

    rerender(
      <MessageAttachment
        attachment={{ id: "a", fileName: "archive.zip", contentType: "application/zip", size: 512, availability: "available" }}
        downloading={false}
        opening={false}
        saving={false}
        revealing={false}
        onOpen={vi.fn()}
        onSaveAs={vi.fn()}
        onReveal={vi.fn()}
      />,
    );
    expect(container.querySelector("svg.lucide-cloud")).toBeNull();
  });
});
