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

## Implementation notes (resolved during apply)

- **Script tests run under the existing vitest setup.** There is no root `package.json`, and a second runner would be a second `pnpm test`. `apps/desktop/vitest.config.ts` therefore also includes `../../scripts/**/*.test.mjs`; those files opt into the node environment with `// @vitest-environment node` (and `src/test/setup.ts` skips its jsdom shim when there is no `window`). `pnpm --filter desktop test` and CI's `pnpm test` step run them with everything else.
- **`bump-version` edits `Cargo.lock` directly** (workspace crates are the `[[package]]` entries without a `source`) instead of running `cargo update -w`, so it is deterministic, offline and testable; the CLI then runs `cargo metadata --offline --no-deps` as the spec's "cargo metadata succeeds" check (`--no-verify` skips it). All new contents are computed before any file is written, so a failure leaves the tree untouched.
- **Minimum macOS is 10.15 (Catalina).** `bundle.macOS.minimumSystemVersion = "10.15"`. Tauri 2's default (10.13) is below what the system webview APIs wry uses support; 10.15 is the oldest release Tauri 2 documents. A local `tauri build --bundles app` confirmed `LSMinimumSystemVersion` 10.15 and `Contents/MacOS/mergeiq` in the bundle. The arm64 slice is inherently 11.0+ (Rust's aarch64-apple-darwin floor); the setting governs the x86_64 slice and the Info.plist check.
- **Bundle metadata.** Explicit targets `dmg, msi, nsis, appimage, deb`; publisher/copyright/license/category, short and long description; `.deb` section `devel` with explicit `libwebkit2gtk-4.1-0` and `libgtk-3-0` dependencies (Tauri adds them too). `ci.yml`'s `tauri build --debug --no-bundle` was re-run locally and stays green.
- **Asset names are Tauri's** (confirmed by a real dry run): `MergeIQ_X.Y.Z_universal.dmg`, `MergeIQ_X.Y.Z_x64_en-US.msi`, `MergeIQ_X.Y.Z_x64-setup.exe`, `MergeIQ_X.Y.Z_amd64.AppImage`, `MergeIQ_X.Y.Z_amd64.deb`. They carry product, version and architecture but no platform word, so the spec requirement was relaxed accordingly (the extension identifies the platform) rather than renaming files that `homebrew-distribution` will reference.
- **Updater artifacts are off.** `tauri-action` otherwise adds `.app.tar.gz`, `.msi.zip` and `.sig` files (the first dry run showed them); `includeUpdaterJson: false` keeps the release to exactly the five installers plus `SHA256SUMS.txt`.
- **The macOS `.app` is deleted after the `.dmg` is built**, so the architecture check mounts the `.dmg` and runs `lipo -archs` on `MergeIQ.app/Contents/MacOS/mergeiq` inside it (result: `x86_64 arm64`).
- **Actions are pinned to commit SHAs** (with the version in a comment) in `release.yml`; majors match what `ci.yml` already uses (checkout/setup-node/pnpm-setup v4, rust-cache v2, tauri-action v0.6.2). Newer majors exist and can be adopted separately. `actionlint` 1.7.12 runs in CI from a pinned Docker image.
- **Signing secrets are exported to the build only when the whole set exists** (macOS: all six; Windows: both), through `$GITHUB_ENV`, because an empty-but-set `APPLE_CERTIFICATE` could be read as "configured". Windows signing imports the PFX and passes a `--config` file with the thumbprint.
- **Verified by a real run** (workflow_dispatch dry run on this branch): the version consistency gate, all three platform builds, the universal `lipo` check, artifact names. **Not verifiable without a tag** (task 4.2): draft creation/reuse, uploading into the draft, asset download, `SHA256SUMS.txt` upload, generated notes, publish and `make_latest`, and both signing paths. Those are covered only by actionlint, shellcheck (via actionlint) and the script unit tests.
- **First real release (v0.1.1-1) lessons.** (1) MSI/WiX only accepts a numeric pre-release identifier, so pre-releases use `X.Y.Z-N` (e.g. `0.1.1-1`), not `-rc.1`. (2) The `build` jobs need `contents: write` to upload into the draft; the dry run cannot catch this because it never uploads. (3) Internal path dependencies must not pin `version`, or a bump to a pre-release breaks resolution. (4) Verified end to end: version gate, draft reuse, upload, `SHA256SUMS.txt` (all five files `OK`), publish as pre-release, dmg `lipo -archs` = `x86_64 arm64`.
