import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import i18n from "@/app/i18n";
import { EditorToolbar } from "./EditorToolbar";

beforeAll(async () => { await i18n.changeLanguage("en-US"); });
afterEach(cleanup);
describe("EditorToolbar lifecycle", () => {
  it("waits for the view to mount before reading cursor styles", async () => {
    const editor = new Editor({ element: null, extensions: [StarterKit], content: "<p>Test</p>" });
    try {
      // This is the real Tiptap state that previously crashed the whole window.
      expect(() => editor.view.domAtPos(1)).toThrow("editor view is not available");
      render(<EditorToolbar editor={editor} sourceMode={false} onEdit={run => run()} onLink={vi.fn()} onRemoveLink={vi.fn()} onSource={vi.fn()} />);
      expect(screen.queryByRole("toolbar")).not.toBeInTheDocument();
      act(() => editor.mount(document.createElement("div")));
      expect(await screen.findByRole("toolbar")).toBeInTheDocument();
      act(() => editor.unmount());
      expect(screen.queryByRole("toolbar")).not.toBeInTheDocument();
    } finally {
      cleanup();
      editor.destroy();
    }
  });
});
