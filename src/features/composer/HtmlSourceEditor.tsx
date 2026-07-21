import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { html } from "@codemirror/lang-html";
import { Compartment, EditorState } from "@codemirror/state";
import {
  drawSelection,
  dropCursor,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { useEffect, useRef } from "react";

interface HtmlSourceEditorProps {
  value: string;
  ariaLabel: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

const editable = new Compartment();

export function HtmlSourceEditor({ value, ariaLabel, disabled, onChange }: HtmlSourceEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!host.current) return;
    const current = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightSpecialChars(),
          history(),
          drawSelection(),
          dropCursor(),
          highlightActiveLine(),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          html(),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": ariaLabel }),
          EditorView.theme({
            "&": { height: "100%", background: "var(--card)", color: "var(--foreground)" },
            ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono, ui-monospace, monospace)" },
            ".cm-gutters": { background: "var(--muted)", color: "var(--muted-foreground)", border: "0" },
            ".cm-activeLine, .cm-activeLineGutter": { background: "color-mix(in srgb, var(--primary) 8%, transparent)" },
            ".cm-content": { caretColor: "var(--foreground)", padding: "12px 0" },
            ".cm-line": { padding: "0 12px" },
            ".cm-cursor": { borderLeftColor: "var(--foreground)" },
            "&.cm-focused": { outline: "none" },
          }),
          editable.of(EditorView.editable.of(!disabled)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onChangeRef.current(update.state.doc.toString());
          }),
        ],
      }),
    });
    view.current = current;
    return () => {
      current.destroy();
      view.current = null;
    };
    // The editor owns its document after mounting; external changes are synchronized below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const current = view.current;
    if (!current || current.state.doc.toString() === value) return;
    current.dispatch({ changes: { from: 0, to: current.state.doc.length, insert: value } });
  }, [value]);

  useEffect(() => {
    view.current?.dispatch({
      effects: editable.reconfigure(EditorView.editable.of(!disabled)),
    });
  }, [disabled]);

  return <div ref={host} className="min-h-0 h-full overflow-hidden" />;
}
