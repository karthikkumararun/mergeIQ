# MergeIQ Roadmap & Handoff

Implement changes **in order**. Each depends on the ones above it. Archive each (`openspec archive <name>`) after it ships so its specs move into `openspec/specs/`.

| # | Change | Milestone | Depends on | Parallelizable with |
|---|--------|-----------|------------|---------------------|
| 1 | `bootstrap-app` | v0.1 | — | — |
| 2 | `merge-engine` | v0.1 | 1 | 3 (after 1) |
| 3 | `git-adapter` | v0.1 | 1, 2 (only `conflict_load` task 5.3) | 2 |
| 4 | `merge-editor-ui` | v0.1 | 2, 3 | — |
| 5 | `mergetool-cli` | v0.1 | 4 | 6 |
| 6 | `repo-browser` | v0.2 | 3, 4, 5 (open cmd) | 5 |
| 7 | `structural-merge` | v0.3 | 2, 4 | 8 |
| 8 | `special-conflicts` | v0.3 | 3, 4, 6 | 7 |
| 9 | `ai-assist` | v0.4 | 2, 3, 4, 7 | — |

## Handoff instructions for the implementing agent

1. Read `openspec/config.yaml` (project context and conventions).
2. For the change: read `proposal.md`, `design.md`, all `specs/**/spec.md`, then work `tasks.md` top to bottom (`/opsx:apply <change>`). If the change has a `ui/` folder, read `openspec/UI.md` first and treat the `ui/*.dc.html` screens and the design.md "UI reference" section as the visual spec.
3. Every spec scenario should map to at least one automated test; name tests after the scenario.
4. Tick `- [x]` per task when its tests pass. Do not mark a task done with failing `cargo test` / `pnpm test`.
5. If a spec is ambiguous or wrong, stop and update the spec (with a note in `design.md` Open Questions) rather than silently deviating.
6. Run `openspec validate <change> --strict` before archiving.

## UI designs

Approved screens: `openspec/changes/<change>/ui/` (exported 2026-10-07). Guide and precedence rules: `openspec/UI.md`. Live canvas: https://claude.ai/artifact/Xrog6wb93Cmt8uN6KQNWb3
