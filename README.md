# MergeIQ

MergeIQ is an open-source, cross-platform desktop app for resolving git merge conflicts. It aims for a
3-way merge editor at least as good as IntelliJ/PyCharm's, plus a repository browser and, later,
AI-assisted conflict resolution (Claude, OpenAI Codex, GitHub Models, Ollama).

MergeIQ works as a standalone app, a `git mergetool`, and (later) a CLI.

## Status

Early development. The desktop shell, typed IPC, theming, settings and CI described below are in place;
the merge engine and editor UI are being built next — see `openspec/ROADMAP.md`.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain — see `rust-toolchain.toml`), with
  the `rustfmt` and `clippy` components.
- [Node.js](https://nodejs.org/) 20+ (see `.nvmrc`) and [pnpm](https://pnpm.io/) 9+.
- [cargo-nextest](https://nexte.st/) for running Rust tests.
- Platform prerequisites for [Tauri 2](https://tauri.app/start/prerequisites/):
  - **macOS**: Xcode Command Line Tools.
  - **Windows**: Microsoft C++ Build Tools (MSVC) and WebView2 (preinstalled on Windows 10/11).
  - **Linux**: see the Tauri docs for your distribution's WebView/GTK packages.

## Building

```sh
pnpm install
pnpm tauri dev      # run the desktop app in development mode
```

Other useful commands:

```sh
cargo fmt --check                        # Rust formatting
cargo clippy --workspace -- -D warnings  # Rust lints
cargo nextest run                        # Rust tests
pnpm --filter desktop lint               # TS/ESLint + stylelint
pnpm --filter desktop test               # Vitest
pnpm tauri build                         # production bundle
```

## Repository layout

```
crates/mergeiq-core     pure merge engine (no IO, no Tauri) — diff3, chunks, auto-resolve
crates/mergeiq-git      git adapter (conflict discovery, stage reads, resolve/stage)
crates/mergeiq-struct   tree-sitter structural merge
crates/mergeiq-ai       AI provider abstraction
apps/desktop/src-tauri  Tauri app, CLI entry, IPC commands
apps/desktop/src        React UI
openspec/               specs, designs and approved UI boards driving development
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please also read our
[Code of Conduct](CODE_OF_CONDUCT.md).

## License

Apache License 2.0 — see [LICENSE](LICENSE).
