## Why

MergeIQ has no code yet. Every later change (merge engine, editor UI, CLI, repo browser) needs a shared, buildable skeleton: Rust workspace, Tauri 2 desktop shell, React UI, typed IPC, CI for macOS + Windows, and open-source hygiene. Establishing it once avoids each feature reinventing structure.

## What Changes

- Create Cargo workspace with empty crates `mergeiq-core`, `mergeiq-git`, `mergeiq-struct`, `mergeiq-ai`.
- Create Tauri 2 app at `apps/desktop` (React 18 + TS strict + Vite + pnpm).
- Typed IPC via `tauri-specta`: TS bindings generated from Rust; a `app_info` command proves the pipeline.
- Light/dark theme tokens (CSS variables) following OS preference with manual override.
- Settings persistence (JSON in OS config dir) for theme and later preferences.
- Structured logging (`tracing`) to rotating file in OS log dir.
- Tooling: rustfmt, clippy (deny warnings), ESLint, Prettier, Vitest, `cargo nextest`.
- GitHub Actions CI matrix (macos-latest, windows-latest, ubuntu-latest): lint, test, build unsigned bundle.
- Release workflow stub (tag-triggered `tauri-action`, signing secrets optional).
- Apache-2.0 LICENSE, README, CONTRIBUTING, CODE_OF_CONDUCT, `.editorconfig`, `.gitattributes`.

## Capabilities

### New Capabilities
- `app-shell`: Desktop application shell — window lifecycle, theming, settings persistence, logging, typed IPC, build/CI.

### Modified Capabilities
<!-- none -->

## Impact

- New repo structure; all later changes build on it.
- Dependencies: tauri 2, tauri-specta, specta, serde, tracing, tracing-appender, directories; react, react-dom, zustand, vite, vitest, typescript.
- Contributors need Rust stable, Node 20+, pnpm 9+, platform Tauri prerequisites (Xcode CLT / MSVC + WebView2).
