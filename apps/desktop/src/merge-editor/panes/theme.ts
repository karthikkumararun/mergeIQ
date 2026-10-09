import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";

const highlight = HighlightStyle.define([
  {
    tag: [
      t.keyword,
      t.modifier,
      t.controlKeyword,
      t.definitionKeyword,
      t.operatorKeyword,
    ],
    color: "var(--kw)",
  },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--type)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--num)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--str)" },
  {
    tag: [t.comment, t.lineComment, t.blockComment],
    color: "var(--faint)",
    fontStyle: "italic",
  },
  { tag: [t.propertyName, t.attributeName], color: "var(--type)" },
]);

/** Pane chrome: fonts, gutters and the chunk classes used by `decorations.ts`. */
export const paneTheme = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "var(--bg)",
    color: "var(--code)",
    fontSize: "13px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "'JetBrains Mono', monospace",
    lineHeight: "22px",
    overflow: "auto",
  },
  ".cm-content": { padding: "0", caretColor: "var(--accent)" },
  ".cm-line": { padding: "0 10px" },
  ".cm-gutters": {
    backgroundColor: "var(--bg)",
    color: "var(--faint)",
    border: "none",
    fontSize: "12px",
  },
  ".cm-lineNumbers .cm-gutterElement": {
    padding: "0 8px 0 4px",
    minWidth: "32px",
  },
  ".cm-merge-marks": { width: "16px" },
  ".cm-merge-marks .cm-gutterElement": {
    textAlign: "center",
    fontWeight: "600",
  },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "transparent" },
  ".cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground":
    {
      backgroundColor: "color-mix(in srgb, var(--accent) 30%, transparent)",
    },
  ".cm-merge-ins": { backgroundColor: "var(--ins-bg)" },
  ".cm-merge-mod": { backgroundColor: "var(--mod-bg)" },
  ".cm-merge-con": { backgroundColor: "var(--con-bg)" },
  ".cm-merge-del": { backgroundColor: "var(--res-bg)" },
  ".cm-merge-res": { backgroundColor: "var(--res-bg)", opacity: "0.72" },
  ".cm-merge-em-ins": { backgroundColor: "var(--ins-em)", borderRadius: "2px" },
  ".cm-merge-em-mod": { backgroundColor: "var(--mod-em)", borderRadius: "2px" },
  ".cm-merge-em-con": { backgroundColor: "var(--con-em)", borderRadius: "2px" },
  ".cm-merge-em-del": { backgroundColor: "var(--res-em)", borderRadius: "2px" },
  ".cm-merge-current": { boxShadow: "inset 0 0 0 1px var(--con-fg)" },
  ".cm-merge-marker": { height: "2px", margin: "0 -10px" },
  ".cm-merge-marker-ins": { backgroundColor: "var(--ins-fg)" },
  ".cm-merge-marker-mod": { backgroundColor: "var(--mod-fg)" },
  ".cm-merge-marker-con": { backgroundColor: "var(--con-fg)" },
  ".cm-merge-marker-del": { backgroundColor: "var(--res-fg)" },
  ".cm-merge-marker-res": { backgroundColor: "var(--res-fg)" },
  ".cm-merge-mark-ins": { color: "var(--ins-fg)" },
  ".cm-merge-mark-mod": { color: "var(--mod-fg)" },
  ".cm-merge-mark-con": { color: "var(--con-fg)" },
  ".cm-merge-mark-del": { color: "var(--res-fg)" },
  ".cm-merge-mark-res": { color: "var(--res-fg)" },
  ".cm-foldPlaceholder": {
    font: "12px 'JetBrains Mono', monospace",
    color: "var(--muted)",
    backgroundColor: "var(--bar)",
    border: "none",
    borderRadius: "4px",
    padding: "4px 8px",
    cursor: "pointer",
  },
});

export const paneHighlight = syntaxHighlighting(highlight);
