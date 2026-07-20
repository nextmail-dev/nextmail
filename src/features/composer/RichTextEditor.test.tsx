import { act, cleanup, render, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import i18n from "@/app/i18n";
import type { DraftContent } from "@/app/types";
import { RichTextEditor, type RichTextEditorHandle } from "./RichTextEditor";

const EMPTY = '{"type":"doc","content":[{"type":"paragraph"}]}';

beforeAll(async () => {
  await i18n.changeLanguage("en-US");
});

afterEach(cleanup);

describe("RichTextEditor composition nodes", () => {
  it("replaces one stable signature node and removes it without touching the body", async () => {
    const ref = createRef<RichTextEditorHandle>();
    const onChange = vi.fn<(content: DraftContent) => void>();
    render(<RichTextEditor ref={ref} initialJson={EMPTY} onChange={onChange} />);
    await waitFor(() => expect(ref.current).not.toBeNull());

    act(() => {
      expect(ref.current?.replaceSignature("signature-one", definition("First"))).toBe(true);
    });
    await waitFor(() => expect(onChange).toHaveBeenCalled());

    act(() => {
      expect(ref.current?.replaceSignature("signature-two", definition("Second"))).toBe(true);
    });
    await waitFor(() => {
      const document = latestDocument(onChange);
      expect(document.content?.filter((node) => node.type === "nextmailSignature")).toHaveLength(1);
      expect(JSON.stringify(document)).toContain("signature-two");
      expect(JSON.stringify(document)).toContain("Second");
      expect(JSON.stringify(document)).not.toContain("signature-one");
    });

    act(() => {
      expect(ref.current?.replaceSignature(null)).toBe(true);
    });
    await waitFor(() => {
      const document = latestDocument(onChange);
      expect(document.content?.some((node) => node.type === "nextmailSignature")).toBe(false);
    });
  });
});

function definition(text: string): DraftContent {
  return {
    editorJson: JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text }] }],
    }),
    html: `<p>${text}</p>`,
    plainText: text,
  };
}

function latestDocument(onChange: ReturnType<typeof vi.fn<(content: DraftContent) => void>>) {
  const call = onChange.mock.calls[onChange.mock.calls.length - 1];
  return JSON.parse(call?.[0].editorJson ?? EMPTY) as {
    content?: Array<{ type?: string }>;
  };
}
