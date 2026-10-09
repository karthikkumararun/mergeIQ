## Why

Most macOS developers install tools with Homebrew. `brew install --cask mergeiq` should install the app and the `mergeiq` command in one step, and stay current without manual work each release.

Depends on: `release-pipeline` (published, checksummed macOS `.dmg`), `mergetool-cli` (the `mergeiq` binary inside the app bundle).

## What Changes

- A separate tap repository `karthikkumararun/homebrew-tap` containing `Casks/mergeiq.rb`, installed with `brew install --cask karthikkumararun/tap/mergeiq`.
- The cask installs `MergeIQ.app` and links `mergeiq` onto the PATH from `MergeIQ.app/Contents/MacOS/mergeiq`; uninstall and zap stanzas remove the app, the link and user data on request.
- A final job in the release workflow that, after a non-pre-release is published, renders the cask from a template in this repo (version, URL, SHA-256 taken from `SHA256SUMS.txt`) and opens/pushes the update to the tap.
- `docs/homebrew.md` (tap layout, token setup, manual update, how to test locally) and a README install line.
- Linux and Windows are out of scope for Homebrew: casks are macOS-only, Linux users use the AppImage/deb, Windows users the installers.

## Capabilities

### New Capabilities
- `homebrew-distribution`: Homebrew cask definition, automated cask updates from releases, install/uninstall behavior.

### Modified Capabilities
<!-- none: the `homebrew` job is added to `release.yml` here and is specified by this change's own requirements -->

## Impact

- New repository `homebrew-tap` (created by the owner), a repo secret `HOMEBREW_TAP_TOKEN` (fine-grained token, contents write on the tap only).
- `.github/workflows/release.yml` (new job), `scripts/release/render-cask.mjs`, `packaging/homebrew/mergeiq.rb.tmpl`, `docs/homebrew.md`.
