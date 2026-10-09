## 1. Version tooling

- [x] 1.1 `scripts/release/read-versions.mjs`: read versions from `tauri.conf.json`, `apps/desktop/package.json`, workspace `Cargo.toml`; vitest-style unit tests for parsing and mismatch reporting
- [x] 1.2 `scripts/release/bump-version.mjs`: semver validation, update the three files and `Cargo.lock` workspace entries; tests for success and invalid input leaving files untouched

## 2. Bundle configuration

- [x] 2.1 `tauri.conf.json`: explicit bundle targets (`dmg`, `msi`, `nsis`, `appimage`, `deb`), macOS minimum version, Linux deb metadata (description, section, depends); `tauri build` debug check stays green in CI
- [ ] 2.2 Confirm the CLI binary is `mergeiq` inside the macOS `.app` (`Contents/MacOS/mergeiq`) and document that path in `docs/releasing.md`

## 3. Workflow

- [ ] 3.1 `release.yml` `verify` job: tag regex, version gate using `read-versions.mjs`, create draft release, output `release_id`, `prerelease`, `signed`
- [ ] 3.2 `build` matrix: macOS universal, Windows, Linux (ubuntu-22.04 + webkit deps) with `tauri-action` uploading to the shared draft; signing env vars optional
- [ ] 3.3 `workflow_dispatch` dry-run path: no release, artifacts uploaded
- [ ] 3.4 `publish` job: download assets, generate `SHA256SUMS.txt`, generated notes + Install section (`notes-template.md`, unsigned notice), upload, publish draft; pre-release handling
- [ ] 3.5 Workflow lint in CI (`actionlint`) so syntax errors are caught on PRs

## 4. Verification and docs

- [ ] 4.1 Dry-run the workflow on a branch and record the result (artifact names, universal arch check via `lipo -archs`) in the PR
- [ ] 4.2 Cut `v0.1.1-rc.1` as the first pre-release; verify downloads and `sha256sum --check` on macOS and Linux
- [ ] 4.3 `docs/releasing.md` runbook (bump, tag, what the workflow does, unsigned caveats, recovering from a failed run) and README Install section
