# Releasing MergeIQ

A release is a `v<semver>` tag. Pushing the tag builds installers for macOS, Windows and Linux,
checks them, and publishes one GitHub Release with checksums and notes. If anything fails, no public
release appears.

## Cut a release

1. **Check CI.** The release workflow does not run the test suite; it relies on `main` being green
   (all three platforms) at the commit you tag.
2. **Bump the version** in one step, on a branch, and merge it:

   ```sh
   node scripts/release/bump-version.mjs 0.2.0      # or 0.2.0-rc.1 for a pre-release
   ```

   This updates `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json`, the workspace
   `Cargo.toml` and the workspace crates in `Cargo.lock`, then checks `cargo metadata`. An argument that
   is not semver (`0.2`, `v0.2.0`) changes nothing.

3. **Tag the merged commit and push the tag:**

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

   Only tags like `v1.2.3` and `v1.2.3-rc.1` start the workflow; any other tag does nothing.

4. **Watch the run** (`gh run watch`). When it finishes the release is public. Install it on one machine
   per platform and run `sha256sum --check SHA256SUMS.txt` before announcing it.

## What the workflow does

`.github/workflows/release.yml`:

| Job       | What it does                                                                                                                                                                                                                                               |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `verify`  | Fails if the tag version differs from `tauri.conf.json`, `apps/desktop/package.json` or `Cargo.toml` (it names each file and the version it contains), detects which signing secrets exist, and creates the **draft** release. Costs seconds.              |
| `build`   | Matrix of macOS (universal arm64 + x86_64), Windows and Linux (`ubuntu-22.04`). Each job uploads its installers to the draft. The macOS job mounts the `.dmg` and runs `lipo -archs` on the `mergeiq` binary inside, failing unless both `arm64` and `x86_64` are present. |
| `publish` | Only after every build job succeeded: checks the asset set, generates `SHA256SUMS.txt` from the files that were uploaded, writes the notes, uploads the checksums, and publishes the draft.                                                                |

Assets for version `X.Y.Z` (names come from Tauri's bundler):

| Platform          | File                                                         |
| ----------------- | ------------------------------------------------------------ |
| macOS (universal) | `MergeIQ_X.Y.Z_universal.dmg`                                |
| Windows           | `MergeIQ_X.Y.Z_x64_en-US.msi`, `MergeIQ_X.Y.Z_x64-setup.exe` |
| Linux             | `MergeIQ_X.Y.Z_amd64.AppImage`, `MergeIQ_X.Y.Z_amd64.deb`    |
| Checksums         | `SHA256SUMS.txt`                                             |

A tag with a suffix (`v0.2.0-rc.1`) publishes as a **pre-release** and is not marked "latest".

Release notes are GitHub's generated notes (merged pull requests since the previous release) followed by an
Install section from `scripts/release/notes-template.md`.

### The CLI inside the app

The `mergeiq` command-line tool is the app binary itself. In the macOS bundle it is
`MergeIQ.app/Contents/MacOS/mergeiq` (Tauri's `mainBinaryName`); Homebrew and **Settings › Command line**
link to that path. The workflow's `lipo` check inspects exactly this file inside the `.dmg`.

### Minimum macOS

`bundle.macOS.minimumSystemVersion` is 10.15 (Catalina). The arm64 slice is inherently macOS 11+.

## Dry run

To test the pipeline without a tag, run it manually from any branch:

```sh
gh workflow run release.yml --ref my-branch
gh run watch
```

A manual run checks that the three version files agree with each other, builds every installer, runs the
macOS architecture check, and attaches the installers to the workflow run as artifacts (`installers-macOS`,
`installers-Windows`, `installers-Linux`). It does not create a release and never publishes.

## Signing

Signing is optional; without the secrets the workflow still succeeds and the notes say which installers are
unsigned and how to open them.

| Platform | Secrets (repository secrets)                                                                                                                                                          |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | All of `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID` — signs and notarizes. |
| Windows  | Both of `WINDOWS_CERTIFICATE` (base64 .pfx) and `WINDOWS_CERTIFICATE_PASSWORD` — Authenticode-signs with a SHA-256 digest.                                                            |

A partial set counts as "not configured" for that platform. Unsigned builds: macOS shows a Gatekeeper warning
(right-click › Open, or `xattr -dr com.apple.quarantine /Applications/MergeIQ.app`); Windows shows SmartScreen
(More info › Run anyway).

## When a run fails

- **Wrong version (`verify` fails).** Nothing was built or created. Delete the tag locally and on the remote,
  fix the versions (`bump-version`), merge, and tag again:
  `git push origin :refs/tags/v0.2.0 && git tag -d v0.2.0`.
- **A build job fails.** The release stays a **draft** and is invisible to users. Fix the cause and use
  **Re-run failed jobs**; the draft is reused. If assets from the failed attempt are already attached, delete
  them from the draft first (they are replaced by name, and a half-uploaded set fails the `publish` asset check).
- **`publish` fails.** The draft still holds all installers. Re-run the job; it regenerates and re-uploads
  `SHA256SUMS.txt` before publishing.
- **Start over.** Delete the draft release on GitHub, delete the tag (above) and tag again. A release that is
  already published is never modified by the workflow.
