# Homebrew distribution

macOS users can install MergeIQ and the `mergeiq` command with one line:

```sh
brew install --cask karthikkumararun/tap/mergeiq
```

The cask installs `MergeIQ.app` into Applications from the release's universal `.dmg` and links
`mergeiq` onto your `PATH` from `MergeIQ.app/Contents/MacOS/mergeiq`. Homebrew casks are macOS-only; on
Linux and Windows use the installers on the [releases page](https://github.com/karthikkumararun/mergeIQ/releases).

| Command                               | Result                                                                          |
| ------------------------------------- | ------------------------------------------------------------------------------- |
| `brew upgrade --cask mergeiq`         | Updates to the newest stable release.                                           |
| `brew uninstall --cask mergeiq`       | Removes the app and the `mergeiq` link.                                         |
| `brew uninstall --zap --cask mergeiq` | Also removes MergeIQ's settings, logs and caches (never your git repositories). |

While the macOS build is not notarized, the cask clears the quarantine attribute itself in a `postflight_steps`
block (`xattr -dr com.apple.quarantine`; Homebrew 7's declarative form, so keep Homebrew up to date), so a Homebrew install opens without the Gatekeeper warning, and
prints a short caveat saying so. This is fine in our own tap; the main `homebrew-cask` repository would not
accept it. A `.dmg` downloaded by hand is still quarantined: right-click the app and choose **Open** once.
Both the postflight and the caveat disappear from the cask once releases are notarized.

## How the tap is kept up to date

The tap is a separate repository, `karthikkumararun/homebrew-tap`, holding only generated output:

```
homebrew-tap/
└── Casks/
    └── mergeiq.rb      # generated; do not edit by hand
```

The cask is **rendered in this repository** from [`packaging/homebrew/mergeiq.rb.tmpl`](../packaging/homebrew/mergeiq.rb.tmpl)
by [`scripts/release/render-cask.mjs`](../scripts/release/render-cask.mjs), so it is reviewed next to the code that
decides the bundle layout. The only inputs are the release version and the `.dmg` line of that release's
`SHA256SUMS.txt`. The renderer refuses pre-release versions (`0.2.0-rc.1`, `0.1.1-1`), a missing `.dmg` entry and a
digest that is not 64 hex characters.

The `homebrew` job in `.github/workflows/release.yml` runs after a stable release is **published**:

1. Skips (with a notice) for pre-releases or when the `HOMEBREW_TAP_TOKEN` secret is missing.
2. Downloads `SHA256SUMS.txt` from the release and renders the cask.
3. Gates it on a macOS runner from a throwaway local tap: `brew style --cask`, `brew audit --cask --online --strict`,
   `brew install --cask`, `mergeiq --version` equals the release version, `brew uninstall --cask`.
4. Pushes one commit titled `mergeiq <version>` that changes only `Casks/mergeiq.rb` (the first push to an empty tap
   creates `main`). Re-running with an unchanged cask pushes nothing.

A failure in this job never unpublishes the GitHub Release. Fix the cause and use **Re-run failed jobs** on the
workflow run to repeat just this job.

## Owner setup (once)

- [ ] Create the empty repository `karthikkumararun/homebrew-tap` (public; Homebrew taps must be named `homebrew-<name>`).
- [ ] Create a **fine-grained personal access token** with access to **only** `karthikkumararun/homebrew-tap` and
      **Contents: Read and write**. Add it to this repository as the Actions secret `HOMEBREW_TAP_TOKEN`
      (Settings › Secrets and variables › Actions).
- [ ] Run `brew search mergeiq` and check that no other cask is named `mergeiq`. If one is, rename the cask (for
      example `mergeiq-app`) in the template and in this document.
- [ ] After the first **stable** release (for example `v0.2.0`), check that the tap has the commit
      `mergeiq 0.2.0`, then on a clean Mac run `brew install --cask karthikkumararun/tap/mergeiq`,
      `mergeiq --version`, `brew uninstall --cask mergeiq` and `brew uninstall --zap --cask mergeiq`.

`brew audit --online` looks up the repository's latest GitHub release, so it can only pass once a stable release
exists (pre-releases are never "latest").

## Update the cask by hand

If the automation is unavailable, render and push the cask yourself from a checkout of this repository:

```sh
gh release download v0.2.0 --pattern SHA256SUMS.txt --repo karthikkumararun/mergeIQ
node scripts/release/render-cask.mjs --version 0.2.0 --checksums SHA256SUMS.txt --out ../homebrew-tap/Casks/mergeiq.rb
# add --signed once the release is notarized (omits the Gatekeeper caveat)
cd ../homebrew-tap && git add Casks/mergeiq.rb && git commit -m "mergeiq 0.2.0" && git push
```

## Test locally

To check a cask before it reaches the tap, use a throwaway local tap (Homebrew no longer installs casks from a bare
file path):

```sh
node scripts/release/render-cask.mjs --version 0.2.0 --checksums SHA256SUMS.txt --out /tmp/mergeiq.rb
brew tap-new --no-git local/mergeiq-test
mkdir -p "$(brew --repository local/mergeiq-test)/Casks"
cp /tmp/mergeiq.rb "$(brew --repository local/mergeiq-test)/Casks/mergeiq.rb"
brew style --cask local/mergeiq-test/mergeiq
brew audit --cask --online --strict local/mergeiq-test/mergeiq
brew install --cask local/mergeiq-test/mergeiq    # add --appdir=<dir> to keep /Applications untouched
brew uninstall --cask local/mergeiq-test/mergeiq
brew untap local/mergeiq-test
```
