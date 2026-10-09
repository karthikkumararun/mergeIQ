## Context

`release.yml` already exists from `bootstrap-app`: it runs `tauri-apps/tauri-action` on macOS and Windows for `v*` tags and creates a draft release. It does not verify versions, build Linux, build a universal macOS binary, produce checksums, or publish. CI (`ci.yml`) is separate and runs on PRs and `main`.

## Goals / Non-Goals

**Goals:**
- A tag produces a complete, verifiable set of downloads or no public release at all.
- Works in a fork without secrets (unsigned).

**Non-Goals:**
- Auto-update (`tauri-plugin-updater`) and update manifests: later change.
- Windows/Linux arm64 builds: later, when runners and demand exist.
- Homebrew, winget, Flatpak: see `homebrew-distribution` for Homebrew; others later.

## Decisions

**Job layout.** `verify` (version gate, runs on ubuntu) → matrix `build` (macOS universal, Windows, Linux) → `publish` (needs all). `verify` fails fast so a wrong tag costs seconds, not three platform builds.

**Draft first, publish last.** `tauri-action` creates one draft release (shared by the three matrix jobs through `releaseId` created by a small step in `verify`, avoiding the matrix race of each job creating its own). The `publish` job runs `gh release edit --draft=false` after uploading `SHA256SUMS.txt` and notes. Failure of any build leaves a draft.

**macOS universal.** `--target universal-apple-darwin` with both Rust targets installed. Bundle target `dmg`; the `.app` inside is what Homebrew links the CLI from.

**Linux.** `ubuntu-22.04` runner (older glibc than latest, so the AppImage/deb run on more distros); installs the same webkit2gtk packages as `ci.yml`. Bundle targets `appimage,deb`.

**Checksums.** The `publish` job downloads the uploaded release assets, runs `sha256sum` on exactly those files, and uploads `SHA256SUMS.txt`. Generating from the released bytes (not build outputs) is what makes the file trustworthy.

**Notes.** `gh release create --generate-notes` equivalent via `gh api repos/.../releases/generate-notes` with `previous_tag_name`; the Install section is appended from a template in `scripts/release/notes-template.md`. Unsigned notice is conditional on secret presence, computed in `verify` and passed as a job output.

**Version helper.** `scripts/release/bump-version` is a small Node script (Node is already required) using regex edits on the three files plus `cargo update -w`. The version gate in `verify` reuses the same read logic via `scripts/release/read-versions.mjs` so the two cannot disagree.

**Dry run.** `workflow_dispatch` skips release creation and `publish`, uploads installers with `actions/upload-artifact`. Used to test the whole pipeline without a tag.

## Risks / Trade-offs

- Unsigned macOS builds trigger Gatekeeper warnings; documented in notes and README. Signing needs an Apple Developer account (owner decision).
- Universal builds roughly double macOS build time; acceptable for a release job.
- Windows tests are currently flaky in CI (known failures); the release workflow does not run tests, it relies on `main` being green. The runbook says to check CI before tagging.

## Open Questions

- Which minimum macOS version? Default to Tauri's (10.13) unless a dependency needs more; set `bundle.macOS.minimumSystemVersion` explicitly after checking.
- Should the Linux `.deb` also be signed or hosted in an apt repo? Not now.
