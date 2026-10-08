## Context

Greenfield. Target macOS 12+ and Windows 10+ (WebView2). Linux builds in CI to keep code portable but is not a release target yet. Later changes add crates and UI modules; this change only lays the skeleton.

## Goals / Non-Goals

**Goals:**
- Reproducible build on all three OSes from a clean checkout with documented prerequisites.
- Typed, generated IPC contract so UI and backend can't drift.
- Workspace layout matching `openspec/config.yaml`.

**Non-Goals:**
- Code signing / notarization (release workflow stub only; secrets wired later).
- Auto-update.
- Any merge functionality.

## Decisions

**Layout**
```
/Cargo.toml                 workspace (members: crates/*, apps/desktop/src-tauri)
/crates/mergeiq-core        lib, no deps beyond std + thiserror for now
/crates/mergeiq-git         lib
/crates/mergeiq-struct      lib
/crates/mergeiq-ai          lib
/apps/desktop/package.json
/apps/desktop/src/          React app
/apps/desktop/src/ipc/bindings.ts   GENERATED (committed)
/apps/desktop/src-tauri/    binary crate `mergeiq-desktop`
/pnpm-workspace.yaml
```
Crate owner of each module: settings + logging live in `src-tauri/src/{settings.rs,logging.rs}`.

**IPC: tauri-specta over hand-written types.** Generated bindings remove a whole class of drift bugs. Bindings exported in debug builds on startup and via a `cargo run -p mergeiq-desktop -- --export-bindings` path (or a test) so CI can check for diffs. Alternative `ts-rs` considered; tauri-specta also generates command wrappers, so chosen.

**State: Zustand.** Small, no boilerplate, works well with per-pane editor state later. Redux rejected as heavy.

**Settings: plain JSON via serde + `directories` crate.** `#[serde(default)]` on struct; unknown keys kept with `#[serde(flatten)] extra: Map<String, Value>`. Atomic writes (write temp + rename).

**Theme tokens.** `src/theme/tokens.css` is a copy of `ui/tokens.css` (approved design): surface/text tokens (`--bg`, `--bar`, `--surface`, `--text`, `--muted`, …), accent, status (`--danger-*`, `--warn-*`, `--ok-*`, `--ai-*`), chunk colours (`--ins-*`, `--mod-*`, `--con-*`, `--res-*`, `--hatch`) and syntax (`--kw`, `--type`, `--str`, `--num`), each defined for `:root[data-theme='dark']` and `:root[data-theme='light']`. `data-theme` attribute on `<html>`. Components use only these variables; hex literals outside `tokens.css` fail lint.

**Fonts.** IBM Plex Sans (UI) and JetBrains Mono (code, paths, SHAs) bundled via `@fontsource/*`; no network font loading.

**Tests.** Rust: `cargo nextest`. UI: Vitest + Testing Library. E2E harness (WebDriver via `tauri-driver`) deferred to `merge-editor-ui`.

## UI reference

Approved screens are in `ui/`; how to read them and precedence rules: `openspec/UI.md`.

- `ui/Tokens.dc.html` + `ui/tokens.css` — the full token set for both themes. `tokens.css` is a superset of the board (adds status and AI colours used by later changes).
- `ui/Main.dc.html` — home window. This change builds the shell: header with logo, "MergeIQ", version chip from `app_info` (`v<version> · <platform>`), and the Light / Dark / System segmented control (also present in Settings › General). The open/drop/recents/error area and the "Set up" cards are built by `repo-browser`, `mergetool-cli` and `ai-assist`; leave a body slot for them.
- The home footer (settings and log paths) is optional polish.

## Risks / Trade-offs

- [WebView differences WKWebView vs WebView2] → keep CSS standard; CI builds both; manual smoke test on each before release.
- [tauri-specta API churn (pre-1.0)] → pin exact versions; isolate usage to `src-tauri/src/ipc.rs`.
- [Windows CI slow] → cache cargo + pnpm stores.
- [`mergeiq-desktop`'s test binary aborts on windows-latest with `0xc0000139` (entry point
  not found) when listed by `cargo nextest`, even in a clean build] → root cause not fully
  isolated; likely from the tauri-template `[lib] crate-type = ["staticlib", "cdylib", "rlib"]`
  combination interacting with the Windows MSVC CRT/test harness. `mergeiq-desktop` tests are
  excluded from `cargo nextest run` on windows-latest only (still compiled there via the later
  `tauri build` step); they run on macOS and Ubuntu. Revisit if nextest or tauri publishes a fix.

## Open Questions

- App identifier: `dev.mergeiq.app` assumed; confirm before first signed release.
