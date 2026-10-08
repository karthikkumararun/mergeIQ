## 1. Repository hygiene

- [ ] 1.1 Add LICENSE (Apache-2.0), README (vision, prerequisites, build steps), CONTRIBUTING, CODE_OF_CONDUCT
- [ ] 1.2 Add `.editorconfig`, `.gitattributes` (`* text=auto`, fixtures `-text` to keep bytes exact), `.gitignore` (target/, node_modules/, dist/)
- [ ] 1.3 Add `rust-toolchain.toml` (stable, components rustfmt + clippy) and `.nvmrc` (20)

## 2. Rust workspace

- [ ] 2.1 Root `Cargo.toml` workspace with shared `[workspace.package]` (version, edition 2021, license) and `[workspace.dependencies]`
- [ ] 2.2 Create lib crates `mergeiq-core`, `mergeiq-git`, `mergeiq-struct`, `mergeiq-ai` each with a placeholder `version()` fn and one unit test
- [ ] 2.3 Verify `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo nextest run` pass

## 3. Tauri desktop app

- [ ] 3.1 Scaffold Tauri 2 app at `apps/desktop` with React + TS + Vite; `pnpm-workspace.yaml` at root
- [ ] 3.2 Configure window: title "MergeIQ", min 1024x640, identifier `dev.mergeiq.app`
- [ ] 3.3 Enable TS `strict`, ESLint (typescript-eslint, react-hooks), Prettier, Vitest + Testing Library
- [ ] 3.4 Add Zustand; create `src/store/` with an app store skeleton

## 4. Typed IPC

- [ ] 4.1 Add `specta` + `tauri-specta`; create `src-tauri/src/ipc.rs` registering commands
- [ ] 4.2 Implement `app_info` command returning name, version, platform
- [ ] 4.3 Export bindings to `apps/desktop/src/ipc/bindings.ts`; add Rust test that regenerates bindings and fails if file changed
- [ ] 4.4 Home view calls `commands.appInfo()` and displays version; Vitest test with mocked bindings

## 5. Settings and logging

- [ ] 5.1 `settings.rs`: Settings struct (`theme: "system"|"light"|"dark"`, flattened extra), load/save with atomic write, corrupt-file backup; unit tests for missing, corrupt, unknown-key cases (use temp dir)
- [ ] 5.2 Commands `get_settings` / `update_settings`
- [ ] 5.3 `logging.rs`: tracing + tracing-appender daily rotation, keep 7, `MERGEIQ_LOG` env filter; log version at startup

## 6. Theming

- [ ] 6.1 `src/theme/tokens.css` copied from `openspec/changes/bootstrap-app/ui/tokens.css` (dark + light); bundle IBM Plex Sans + JetBrains Mono via `@fontsource/*`
- [ ] 6.2 Theme hook: applies `data-theme`, listens to `prefers-color-scheme` in system mode, persists via settings
- [ ] 6.3 Settings view with theme selector; Vitest test for system-mode switching
- [ ] 6.4 Home shell per `ui/Main.dc.html` (logo, name, version chip from `app_info`, Light/Dark/System control, empty body slot); Vitest render test asserting name, version and theme control
- [ ] 6.5 Lint rule (stylelint `color-no-hex` or equivalent) failing on hex colours outside `src/theme/tokens.css`; wired into `pnpm lint`

## 7. CI and release

- [ ] 7.1 `.github/workflows/ci.yml`: matrix macOS/Windows/Ubuntu; cache cargo + pnpm; fmt, clippy, nextest, bindings check, pnpm lint/test, tauri debug build
- [ ] 7.2 `.github/workflows/release.yml`: tag `v*` → `tauri-action` builds dmg + msi as draft release (signing env vars optional, skipped when absent)
- [ ] 7.3 Confirm CI green on all three OSes
