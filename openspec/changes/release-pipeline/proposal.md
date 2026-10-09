## Why

MergeIQ can only be run from source today. A minimal `release.yml` exists (macOS + Windows, draft release, no verification), but nothing guarantees that a tag produces a complete, trustworthy set of downloads. Users need to download an installer for their OS from GitHub Releases, check its integrity, and trust that the version they got matches the tag.

Depends on: `bootstrap-app` (Tauri bundling, CI), `mergetool-cli` (the `mergeiq` binary name).

## What Changes

- Harden `.github/workflows/release.yml`, triggered by a `v<semver>` tag (and `workflow_dispatch` for dry runs):
  - Verify the tag matches the version in `tauri.conf.json`, `apps/desktop/package.json` and the workspace `Cargo.toml` before building anything.
  - Build installers on three platforms: macOS universal `.dmg` (arm64 + x64), Windows `.msi` and NSIS `.exe`, Linux `.AppImage` and `.deb`.
  - Upload every installer plus `SHA256SUMS.txt` to one GitHub Release, with generated release notes.
  - Create the release as a draft, publish it only after all platform jobs succeed; pre-release tags (`v1.2.3-rc.1`) publish as pre-releases.
  - Signing and notarization are optional: use secrets when present, otherwise build unsigned and say so in the release notes.
- A `scripts/release/` helper to bump the version in all three files together, and a `docs/releasing.md` runbook.
- README "Install" section pointing to the releases page.

## Capabilities

### New Capabilities
- `release-pipeline`: Tag-driven build, verification, checksum and publication of installers to GitHub Releases.

### Modified Capabilities
<!-- none -->

## Impact

- `.github/workflows/release.yml`, `scripts/release/`, `docs/releasing.md`, `README.md`.
- `tauri.conf.json` bundle settings (targets, macOS minimum version, Linux deb metadata).
- Needs repo secrets for signing (optional) and nothing else; uses the built-in `GITHUB_TOKEN`.
- Consumed by `homebrew-distribution`, which reads the published asset URLs and checksums.
