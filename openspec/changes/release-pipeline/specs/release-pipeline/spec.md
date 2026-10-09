## ADDED Requirements

### Requirement: Tag-triggered release
Pushing a tag matching `v<MAJOR>.<MINOR>.<PATCH>` with an optional `-<prerelease>` suffix SHALL start the release workflow. Other tags SHALL NOT start it. A manual `workflow_dispatch` run SHALL build all installers and upload them as workflow artifacts without creating or modifying a GitHub Release.

#### Scenario: Release tag
- **WHEN** the tag `v0.2.0` is pushed
- **THEN** the release workflow starts and builds installers for every supported platform

#### Scenario: Non-release tag
- **WHEN** the tag `nightly-test` is pushed
- **THEN** the release workflow does not start

#### Scenario: Dry run
- **WHEN** the workflow is started manually
- **THEN** installers are attached as workflow artifacts and no GitHub Release is created

### Requirement: Version consistency gate
Before any installer is built, the workflow SHALL fail if the tag version (without the leading `v`) differs from the version in `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json` or the workspace `Cargo.toml`. The failure message SHALL name each file and the version it contains.

#### Scenario: Mismatched version
- **WHEN** tag `v0.2.0` is pushed while `tauri.conf.json` says `0.1.0`
- **THEN** the workflow fails in its first job, names `tauri.conf.json` and `0.1.0`, and builds nothing

#### Scenario: Matching version
- **WHEN** tag `v0.2.0` is pushed and all three files say `0.2.0`
- **THEN** the gate passes and the build jobs start

### Requirement: Installers per platform
A successful release SHALL contain, for version `X.Y.Z`: a macOS universal `.dmg` (arm64 and x86_64 in one app), a Windows `.msi` and an NSIS `.exe` for x86_64, and a Linux x86_64 `.AppImage` and `.deb`. Asset file names SHALL include the product name, version, platform and architecture.

#### Scenario: Complete asset set
- **WHEN** a release for `v0.2.0` is published
- **THEN** it lists exactly one `.dmg`, one `.msi`, one `.exe`, one `.AppImage` and one `.deb`, each containing `0.2.0` in the file name

#### Scenario: macOS universal
- **WHEN** the `.dmg` is inspected after build
- **THEN** the contained `mergeiq` binary reports both `arm64` and `x86_64` architectures

### Requirement: Checksums
The release SHALL include `SHA256SUMS.txt` listing the SHA-256 digest and file name of every installer asset, in the format accepted by `sha256sum --check`. The file SHALL be generated from the exact files uploaded.

#### Scenario: Verify a download
- **WHEN** a user downloads all assets and runs `sha256sum --check SHA256SUMS.txt`
- **THEN** every listed file reports `OK`

### Requirement: Atomic publication
The workflow SHALL create the GitHub Release as a draft and publish it only after every platform job and the checksum job have succeeded. If any job fails, the release SHALL remain a draft (or not exist) and SHALL NOT be visible on the public releases page. A tag with a prerelease suffix SHALL publish as a pre-release and SHALL NOT be marked "latest".

#### Scenario: One platform fails
- **WHEN** the Linux build fails and macOS and Windows succeed
- **THEN** the release stays a draft and the workflow run is marked failed

#### Scenario: Pre-release tag
- **WHEN** tag `v0.2.0-rc.1` is built successfully
- **THEN** the release is published as a pre-release and is not marked latest

### Requirement: Release notes
The published release SHALL have notes generated from the merged pull requests since the previous release tag, followed by an Install section listing the asset for each platform and, when any installer is unsigned, a notice naming which platforms are unsigned and how to open them.

#### Scenario: Unsigned build notice
- **WHEN** no Apple signing secrets are configured
- **THEN** the release notes state that the macOS build is unsigned and describe the Gatekeeper override

### Requirement: Optional signing
macOS codesigning and notarization, and Windows Authenticode signing, SHALL be applied only when their secrets are present. Absence of secrets SHALL NOT fail the build.

#### Scenario: No secrets
- **WHEN** the workflow runs in a fork without signing secrets
- **THEN** unsigned installers are produced and the workflow succeeds

### Requirement: Version bump helper
`scripts/release/bump-version <X.Y.Z>` SHALL update the version in `tauri.conf.json`, `apps/desktop/package.json` and the workspace `Cargo.toml` (and `Cargo.lock` for workspace crates) in one step, and SHALL refuse an argument that is not valid semver.

#### Scenario: Bump
- **WHEN** `bump-version 0.2.0` runs
- **THEN** all three files report `0.2.0` and `cargo metadata` succeeds

#### Scenario: Invalid argument
- **WHEN** `bump-version 0.2` runs
- **THEN** it exits non-zero with a message and changes no file
