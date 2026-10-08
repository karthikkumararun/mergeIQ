# Contributing to MergeIQ

Thanks for your interest in contributing!

## Workflow

MergeIQ is developed spec-first with [OpenSpec](https://github.com/Fission-AI/OpenSpec):
proposals, design and specs live under `openspec/changes/<change>/` before implementation.
Check `openspec/ROADMAP.md` for the current change order.

1. Open an issue or check `openspec/changes/` for an existing proposal before starting work.
2. Set up your environment per the [README prerequisites](README.md#prerequisites).
3. Create a branch, make your changes, and ensure the checks below pass locally.
4. Open a pull request against `main`. CI runs on macOS, Windows and Linux.

## Checks before opening a PR

```sh
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo nextest run
pnpm --filter desktop lint
pnpm --filter desktop test
```

If you change any `#[tauri::command]`, regenerate the TypeScript bindings (`apps/desktop/src/ipc/bindings.ts`)
and commit the diff — CI fails if bindings are stale.

## Code style

- Rust: `rustfmt` defaults; `thiserror` for library errors, `anyhow` only in the app binary; no panics
  on user data.
- TypeScript: strict mode; no hand-written `invoke()` calls — use the generated `commands` bindings.
- UI colors come only from CSS variables in `apps/desktop/src/theme/tokens.css` — no hard-coded hex values.

## Code of Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to abide by it.
