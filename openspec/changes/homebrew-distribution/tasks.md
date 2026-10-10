## 1. Cask template and renderer

- [x] 1.1 `packaging/homebrew/mergeiq.rb.tmpl` with version/sha256/url placeholders, `app`, `binary`, `zap`, `livecheck`, optional unsigned caveat; confirm asset name and zap paths against a real `release-pipeline` build
- [x] 1.2 `scripts/release/render-cask.mjs`: inputs version, checksums file, signed flag; fails on pre-release version, missing `.dmg` entry, bad digest; tests for each scenario (render, pre-release refused, missing checksum, signed vs unsigned caveat)

## 2. Workflow integration

- [x] 2.1 `release.yml` `homebrew` job (needs `publish`, macOS runner): skip on pre-release or missing `HOMEBREW_TAP_TOKEN`; render cask
- [x] 2.2 Gate: `brew style`, `brew audit --cask --online --strict`, and `brew install --cask` / `mergeiq --version` / `brew uninstall` from a temporary local tap
- [x] 2.3 Push one commit `mergeiq <version>` to the tap touching only `Casks/mergeiq.rb`; job is independently re-runnable and never unpublishes the release

## 3. Docs and first publish

- [x] 3.1 `docs/homebrew.md`: tap layout, creating the token, manual cask update, local testing with `brew install --cask ./Casks/mergeiq.rb`; README install line
- [x] 3.2 Owner steps (checklist in the doc): create `karthikkumararun/homebrew-tap`, add `HOMEBREW_TAP_TOKEN`, check `brew search mergeiq` for name collision
- [ ] 3.3 First end-to-end run on a stable tag; verify `brew install --cask karthikkumararun/tap/mergeiq`, `mergeiq --version`, uninstall and zap on a clean Mac
