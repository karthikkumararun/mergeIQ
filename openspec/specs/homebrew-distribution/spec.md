# homebrew-distribution Specification

## Purpose
TBD - created by archiving change homebrew-distribution. Update Purpose after archive.

## Requirements

### Requirement: Cask installs the app and CLI
The Homebrew cask `mergeiq` SHALL install `MergeIQ.app` into the Applications directory from the release `.dmg` of the same version and SHALL link the `mergeiq` command onto the PATH from `MergeIQ.app/Contents/MacOS/mergeiq`.

#### Scenario: Install
- **WHEN** a user runs `brew install --cask karthikkumararun/tap/mergeiq`
- **THEN** `MergeIQ.app` is installed and `mergeiq --version` prints the cask's version

#### Scenario: Version matches release
- **WHEN** the cask for `0.2.0` is installed
- **THEN** the downloaded file is the `.dmg` asset of release `v0.2.0` and its SHA-256 equals the entry in that release's `SHA256SUMS.txt`

### Requirement: Clean uninstall
`brew uninstall --cask mergeiq` SHALL remove the app and the `mergeiq` link. `brew uninstall --zap --cask mergeiq` SHALL additionally remove MergeIQ's settings and caches in the user's library directories and SHALL NOT touch any git repository.

#### Scenario: Uninstall
- **WHEN** the user runs `brew uninstall --cask mergeiq`
- **THEN** `MergeIQ.app` and the `mergeiq` link no longer exist

#### Scenario: Zap
- **WHEN** the user runs `brew uninstall --zap --cask mergeiq`
- **THEN** the settings file and caches under the user's library directories are removed

### Requirement: Cask is generated from the release
The cask file SHALL be rendered by `scripts/release/render-cask.mjs` from `packaging/homebrew/mergeiq.rb.tmpl` using only the release version and the `.dmg` entry of `SHA256SUMS.txt`. Rendering SHALL fail if the version has a prerelease suffix, if the `.dmg` entry is missing, or if the digest is not 64 hex characters.

#### Scenario: Render
- **WHEN** the script runs with version `0.2.0` and a checksums file containing the `.dmg`
- **THEN** the output cask has `version "0.2.0"`, the matching `sha256`, and a `url` pointing at the `v0.2.0` release asset

#### Scenario: Pre-release refused
- **WHEN** the script runs with version `0.2.0-rc.1`
- **THEN** it exits non-zero and writes no file

#### Scenario: Missing checksum
- **WHEN** the checksums file has no `.dmg` entry
- **THEN** it exits non-zero naming the missing asset

### Requirement: Automatic tap update
After a non-pre-release GitHub Release is published, the release workflow SHALL update `Casks/mergeiq.rb` in the tap repository with the rendered cask in a single commit titled `mergeiq <version>`. Pre-releases SHALL NOT update the tap. A failure to update the tap SHALL NOT unpublish or fail the GitHub Release itself, but SHALL mark the workflow run with a visible warning and a re-runnable job.

#### Scenario: Stable release
- **WHEN** release `v0.2.0` is published
- **THEN** the tap gains one commit `mergeiq 0.2.0` changing only `Casks/mergeiq.rb`

#### Scenario: Pre-release
- **WHEN** release `v0.2.0-rc.1` is published
- **THEN** the tap is not modified

#### Scenario: Tap token missing
- **WHEN** `HOMEBREW_TAP_TOKEN` is not configured
- **THEN** the homebrew job is skipped with a notice and the release stays published

### Requirement: Cask passes Homebrew audit
The rendered cask SHALL pass `brew audit --cask --online --strict` and `brew style` and SHALL install successfully in CI on a macOS runner before the tap commit is pushed.

#### Scenario: Audit gate
- **WHEN** the rendered cask fails `brew audit`
- **THEN** the tap is not updated and the job fails with the audit output

### Requirement: Unsigned build guidance
While the macOS app is not notarized, the cask SHALL include a `caveats` message explaining the Gatekeeper warning and the exact command to clear the quarantine attribute; once the release is notarized the caveat SHALL be omitted.

#### Scenario: Unsigned caveat
- **WHEN** the release was produced without Apple signing secrets
- **THEN** the rendered cask contains the Gatekeeper caveat

#### Scenario: Notarized
- **WHEN** the release was signed and notarized
- **THEN** the rendered cask contains no caveat
