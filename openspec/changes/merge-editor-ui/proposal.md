## Why

This is the product. VS Code's merge editor lacks IntelliJ's clarity: three synchronized panes joined by connector bands, one-click `>>`/`X` per change, "Apply non-conflicting changes", the magic-wand "Resolve simple conflicts", word-level highlights and a live counter of what's left. MergeIQ must match that experience and improve on labeling (who is each side) and keyboard flow.

Depends on: `bootstrap-app`, `merge-engine`, `git-adapter` (for `conflict_load` / `conflict_save` IPC shapes).

## What Changes

- Reusable `<MergeEditor>` React component fed by a `MergeDocument` (Analysis, labels, commit context) and save/cancel callbacks — used by both `mergetool-cli` and `repo-browser`.
- Three CodeMirror 6 panes: left (ours, read-only), center (result, editable), right (theirs, read-only); optional read-only base pane.
- Connector gutters drawing bands between corresponding chunks with per-chunk action buttons (apply, append, ignore).
- Chunk session model in CodeMirror state: per-side status, result-range tracking through edits, fully undoable.
- Toolbar: apply non-conflicting (all / left / right), resolve simple conflicts, accept whole left/right, prev/next change and conflict, whitespace policy, show base, collapse unchanged, counter.
- Side headers with contextual labels and commit-context popovers.
- Syntax highlighting for Java, Python, Kotlin, YAML, JSON, JavaScript, TypeScript, Go (+ plain text fallback).
- Synchronized scrolling aligned by chunks.
- Keyboard shortcuts for every action; accessible non-color indicators.
- Save flow with unresolved-conflict dialog; cancel with discard confirmation.

## Capabilities

### New Capabilities
- `merge-editor`: Interactive three-pane merge resolution UI and its session semantics.

### Modified Capabilities
<!-- none -->

## Impact

- New code under `apps/desktop/src/merge-editor/`.
- Dependencies: `@codemirror/state`, `view`, `commands`, `search`, `language`, `lang-java`, `lang-python`, `lang-javascript` (JS/TS), `lang-json`, `lang-yaml`, `@codemirror/legacy-modes` (Kotlin, Go) or `lang-go`; Playwright for UI tests.
- Settings additions: `whitespacePolicy`, `showBase`, `autoApplyNonConflicting`, `collapseUnchanged`.
