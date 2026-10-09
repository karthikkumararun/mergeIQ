import { describe, expect, it } from "vitest";
import { fixture } from "../__fixtures__";
import { applySide, ignoreSide } from "./actions";
import { buildSaveResult, renderSaveText } from "./serialize";
import { createResultState } from "./state";
import type { MergeLabels } from "./types";
import type { EditorState, TransactionSpec } from "@codemirror/state";

const labels: MergeLabels = {
  left: {
    role: "Your branch",
    refName: "main",
    shortSha: null,
    subject: null,
    author: null,
    gitTerm: "ours",
  },
  right: {
    role: "Incoming",
    refName: "feature",
    shortSha: null,
    subject: null,
    author: null,
    gitTerm: "theirs",
  },
};

const run = (s: EditorState, spec: TransactionSpec | null) =>
  s.update(spec!).state;

describe("Save and cancel", () => {
  it("Clean save", () => {
    const a = fixture("simple-conflict");
    let s = createResultState(a);
    s = run(s, applySide(s, a, 0, "left"));
    s = run(s, ignoreSide(s, a, 0, "right"));
    const result = buildSaveResult(s, a, labels, "resolved");
    expect(result.unresolvedIds).toEqual([]);
    expect(renderSaveText(result)).toBe("line1\nOURS\nline3\n");
  });

  it("Save with unresolved", () => {
    const a = fixture("mixed-changes");
    const s = createResultState(a);
    const result = buildSaveResult(s, a, labels, "markers");
    expect(result.unresolvedIds).toEqual([0, 1, 2]);
    expect(renderSaveText(result)).toBe(
      "l1\nl2\nl3\nl4\n<<<<<<< main\nOURS5\n=======\nTHEIRS5\n>>>>>>> feature\nl6\nl7\nl8\nl9\nl10\n",
    );
  });

  it("Force mode keeps the current result for unresolved chunks", () => {
    const a = fixture("simple-conflict");
    const s = createResultState(a);
    const result = buildSaveResult(s, a, labels, "force");
    expect(result.mode).toBe("force");
    expect(renderSaveText(result)).toBe(a.base.text);
  });

  it("Preserves CRLF line endings", () => {
    const a = fixture("crlf-line-endings");
    const s = createResultState(a);
    expect(renderSaveText(buildSaveResult(s, a, labels, "resolved"))).toBe(
      a.base.text,
    );
    expect(a.base.text).toContain("\r\n");
  });

  it("Drops the final newline when no side had one", () => {
    const a = {
      ...fixture("simple-conflict"),
    };
    const noEol = {
      ...a,
      base: {
        ...a.base,
        text: "line1\nline2\nline3",
        lines: a.base.lines.map((l, i) =>
          i === 2 ? { ...l, term: "None" as const } : l,
        ),
      },
      ours: {
        ...a.ours,
        lines: a.ours.lines.map((l, i) =>
          i === 2 ? { ...l, term: "None" as const } : l,
        ),
      },
      theirs: {
        ...a.theirs,
        lines: a.theirs.lines.map((l, i) =>
          i === 2 ? { ...l, term: "None" as const } : l,
        ),
      },
    };
    const s = createResultState(noEol);
    expect(renderSaveText(buildSaveResult(s, noEol, labels, "force"))).toBe(
      "line1\nline2\nline3",
    );
  });
});
