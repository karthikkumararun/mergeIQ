## Context

Homebrew casks install prebuilt macOS apps from a URL with a checksum. Casks are macOS-only; the main `homebrew-cask` repository requires notarized apps and notable usage, so the first home is our own tap. The release pipeline already yields a universal `.dmg` and `SHA256SUMS.txt`.

## Goals / Non-Goals

**Goals:**
- `brew install --cask karthikkumararun/tap/mergeiq` gives app + `mergeiq` CLI.
- No manual cask edits per release.

**Non-Goals:**
- Submission to `homebrew/homebrew-cask` (needs notarization and traction; revisit).
- A Linux formula or Linuxbrew support (casks do not exist there).
- winget/Scoop/Chocolatey (separate future change).

## Decisions

**Own tap, one cask.** Repo `karthikkumararun/homebrew-tap`, file `Casks/mergeiq.rb`. The owner creates the empty repo; this change does not create repositories.

**Cask shape.**
```ruby
cask "mergeiq" do
  version "X.Y.Z"
  sha256 "<digest>"
  url "https://github.com/karthikkumararun/mergeIQ/releases/download/v#{version}/MergeIQ_#{version}_universal.dmg"
  name "MergeIQ"
  desc "Merge conflict resolver with a three-pane editor"
  homepage "https://github.com/karthikkumararun/mergeIQ"
  depends_on :macos
  app "MergeIQ.app"
  binary "#{appdir}/MergeIQ.app/Contents/MacOS/mergeiq"
  zap trash: ["~/Library/Application Support/dev.mergeiq.app", "~/Library/Caches/dev.mergeiq.app"]
end
```
Exact asset file name and `zap` paths are confirmed against a real build in task 1.1 (the `release-pipeline` naming rule decides the file name).

**Rendering lives in this repo.** `packaging/homebrew/mergeiq.rb.tmpl` plus `scripts/release/render-cask.mjs` (Node, unit tested). The tap holds only generated output, so the template is reviewed alongside the code that determines the bundle layout.

**Workflow job.** `homebrew` job `needs: publish`, runs on `macos-latest`: render cask → `brew audit --cask --online --strict` and `brew install --cask` from a temporary local tap → clone tap with `HOMEBREW_TAP_TOKEN`, commit, push to the tap's default branch. Skipped when the tag has a prerelease suffix or the secret is absent. It is a separate job so it can be re-run alone with "Re-run failed jobs".

**Token.** Fine-grained personal access token limited to the tap repo with contents read/write, stored as `HOMEBREW_TAP_TOKEN`. A GitHub App would be cleaner but is heavier for one repo.

**Caveat logic.** `signed` output from `release-pipeline`'s `verify` job is passed to the renderer, which adds the quarantine caveat only when unsigned.

## Risks / Trade-offs

- Unsigned apps: users must clear quarantine or right-click open; the caveat tells them how. Notarization removes this and is the main follow-up.
- A broken cask pushed to the tap affects installs immediately: mitigated by the audit and install gate before pushing.
- Binary link path depends on the bundle layout; a test in `release-pipeline` task 2.2 already asserts `Contents/MacOS/mergeiq` exists.

## Open Questions

- Cask name collision: check `brew search mergeiq` before the first publish; fall back to `mergeiq-app` if taken.
- Add `auto_updates`/livecheck stanza now or when the in-app updater exists? Use `livecheck` against GitHub releases now; revisit with the updater.

## Implementation notes (resolved during apply)

- **No macOS version floor in the cask.** The design's `depends_on macos: ">= :high_sierra"` is wrong twice: the bundle's minimum is 10.15 (`bundle.macOS.minimumSystemVersion`), and Homebrew 7's audit rejects `depends_on macos: :catalina` as disabled ("There is no replacement"), while `brew style` asks for a bare `depends_on :macos` on macOS-only casks. The template uses `depends_on :macos`; older systems are stopped by the app's own `LSMinimumSystemVersion`.
- **Asset and bundle facts confirmed against the real release v0.1.1-1:** the asset is `MergeIQ_<version>_universal.dmg`, it contains `MergeIQ.app/Contents/MacOS/mergeiq`, `mergeiq --version` prints `mergeiq <version>`, and `SHA256SUMS.txt` lines are `<sha256>  <name>`. The bundle id is `dev.mergeiq.app`.
- **`zap`** removes `~/Library/Application Support/dev.mergeiq.app` and `…/MergeIQ` (the settings file is under the latter: `directories` config dir + `MergeIQ`), `~/Library/Logs/MergeIQ` (the log directory in `logging.rs`), and the bundle-id caches/prefs/saved state/WebKit data. Repositories are never touched.
- **`livecheck`** uses `strategy :github_latest`: `release-pipeline` marks only stable releases as latest, so pre-releases are invisible to it. (A repository with no stable release yet makes `brew audit --online` raise a GitHub 404 for that lookup; it clears with the first stable release.)
- **Renderer refuses anything but `MAJOR.MINOR.PATCH`.** That includes the numeric pre-release tags the project uses (`0.1.1-1`). `fillTemplate` is exported without validation only so a local audit experiment can render a cask from the real pre-release's checksums.
- **Local validation (Homebrew 7.0.9, cask rendered from the real v0.1.1-1 checksums into a throwaway local tap):** `brew style --cask` reports no offenses for the signed and unsigned renderings; `brew audit --cask --online --strict` reports only "v0.1.1-1 is a GitHub pre-release" (expected for this experiment) and the GitHub 404 above; `brew install --cask` (with `--appdir` in a temp directory) linked `mergeiq`, `mergeiq --version` printed `mergeiq 0.1.1-1`, and `brew uninstall --cask` removed the app and the link. The throwaway tap was removed afterwards.
- **Local testing uses a throwaway local tap**, not `brew install --cask ./Casks/mergeiq.rb` as task 3.1 says: Homebrew 7 rejects it ("Homebrew requires casks to be in a tap"), verified locally. `docs/homebrew.md` documents the `brew tap-new --no-git local/...` recipe (the same one the workflow gate uses).
- **Workflow `homebrew` job** (`needs: [verify, publish]`, `macos-latest`, skipped for pre-releases via the job condition and for a missing token via a step output, with a notice): download `SHA256SUMS.txt` with `gh release download`, render, gate (style, audit `--online --strict`, install, `mergeiq --version` equals `mergeiq <version>`, uninstall leaves no app or link, clearing quarantine first only for an unsigned release as the caveat instructs), then clone the tap with an `http.extraheader` (the token never appears in the URL; the encoded value is masked), create `main` if the tap is empty, stage only `Casks/mergeiq.rb`, refuse if anything else is staged, skip when unchanged, commit `mergeiq <version>` and push. The clone/commit/push logic was exercised locally against an empty bare repository (first push creates `main` with one commit touching only the cask; a second run pushes nothing).
- **Failure semantics.** The job runs after `publish`, so a failure cannot unpublish anything; it fails visibly (red job, workflow marked failed) and "Re-run failed jobs" repeats only this job. No dry-run path was added: a manual run builds installers but has no published release to render from.
- **Not verifiable without a stable release** (task 3.3): the real tap push (token, permissions, empty-tap clone against GitHub), `brew audit --online` clean (needs a non-pre-release "latest" release), the install gate on the `macos-latest` runner (Gatekeeper behaviour of the unsigned app, `/Applications` write access), and `brew install --cask karthikkumararun/tap/mergeiq` on a clean Mac.
- **Verified on the first stable release (v0.2.0).** The release workflow pushed one commit `mergeiq 0.2.0` to the tap changing only `Casks/mergeiq.rb`; `brew install --cask karthikkumararun/tap/mergeiq` downloaded the dmg, installed `MergeIQ.app`, linked `mergeiq`, printed the Gatekeeper caveat, and `mergeiq --version` printed `mergeiq 0.2.0`. `brew search` only finds the cask after the tap is added (install adds it).
