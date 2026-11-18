import { useEffect, useRef } from "react";
import { Compartment, EditorState } from "@codemirror/state";
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  placeholder as cmPlaceholder,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { json } from "@codemirror/lang-json";

const mono = '"SF Mono", ui-monospace, "JetBrains Mono", Menlo, monospace';

const theme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "12.5px",
    backgroundColor: "transparent",
    color: "#1a1a18",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: mono,
    lineHeight: "1.7",
    overflow: "auto",
    padding: "6px 0 16px",
  },
  ".cm-content": { caretColor: "#0e7c66", paddingRight: "12px" },
  ".cm-line": { paddingLeft: "4px" },
  ".cm-gutters": {
    backgroundColor: "transparent",
    border: "none",
    color: "#b9b5ae",
    fontSize: "11px",
    paddingRight: "4px",
  },
  ".cm-activeLine": { backgroundColor: "#0e7c6609" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "#0e7c66" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "#cde7df !important",
  },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "#0e7c66", borderLeftWidth: "2px" },
  ".cm-placeholder": { color: "#b9b5ae" },
  ".cm-foldPlaceholder": {
    backgroundColor: "#f0eee9",
    border: "none",
    color: "#76736d",
    padding: "0 4px",
    borderRadius: "3px",
  },
});

type Props = {
  value: string;
  onChange?: (v: string) => void;
  language?: "json" | "text";
  readOnly?: boolean;
  placeholder?: string;
};

export default function CodeEditor({ value, onChange, language = "json", readOnly, placeholder }: Props) {
  const host = useRef<HTMLDivElement | null>(null);
  const view = useRef<EditorView | null>(null);
  const ro = useRef(new Compartment());
  const emit = useRef(onChange);
  emit.current = onChange;

  useEffect(() => {
    if (!host.current) return;

    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        drawSelection(),
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        language === "json" ? json() : [],
        cmPlaceholder(placeholder ?? ""),
        EditorView.lineWrapping,
        theme,
        ro.current.of([EditorState.readOnly.of(!!readOnly), EditorView.editable.of(!readOnly)]),
        EditorView.updateListener.of((u) => {
          if (u.docChanged && emit.current) emit.current(u.state.doc.toString());
        }),
      ],
    });

    const v = new EditorView({ state, parent: host.current });
    view.current = v;
    return () => {
      v.destroy();
      view.current = null;
    };
    // 只在 language 变化时重建；其余属性由下面的 effect 同步
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [language]);

  useEffect(() => {
    const v = view.current;
    if (!v) return;
    if (value !== v.state.doc.toString()) {
      v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: value } });
    }
  }, [value]);

  useEffect(() => {
    view.current?.dispatch({
      effects: ro.current.reconfigure([
        EditorState.readOnly.of(!!readOnly),
        EditorView.editable.of(!readOnly),
      ]),
    });
  }, [readOnly]);

  return <div ref={host} className="h-full w-full overflow-hidden" />;
}
