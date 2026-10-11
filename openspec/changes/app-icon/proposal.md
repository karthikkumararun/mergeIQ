## Why

MergeIQ still ships the stock Tauri logo: the Dock, Finder, Start menu, taskbar and the Homebrew-installed app all show it, so the app looks unfinished and is hard to pick out among other windows. It needs its own recognizable icon, derived from the mark the UI already uses.

Depends on: `bootstrap-app` (Tauri bundling), `release-pipeline` (installers built from the committed icons).

## What Changes

- A designed app icon, concept "A · Merge Y": two amber branches merging into one stem on a dark rounded square, from the approved Claude Design canvas (https://claude.ai/artifact/WBeVGgA85r1U9GCZhsxC5k).
- One committed master, `assets/icon/mergeiq-icon.svg`, and a reproducible build script `scripts/icons/build-icons.mjs` that renders it to a 1024 px PNG and runs `tauri icon` to regenerate every platform file in `apps/desktop/src-tauri/icons/` (`icon.icns`, `icon.ico`, PNG sizes, Windows Store logos).
- The web favicon (`apps/desktop/public/icon.png`) uses the same artwork.
- A guard test that fails if the stock Tauri icons come back or the generated files are missing or the wrong size.
- Design notes live in this change; the approved board is linked, not exported, as with earlier UI changes.

## Capabilities

### New Capabilities
- `app-icon`: Brand icon artwork, how it is generated, and what is bundled on each platform.

### Modified Capabilities
<!-- none -->

## Impact

- `assets/icon/`, `scripts/icons/`, `apps/desktop/src-tauri/icons/*`, `apps/desktop/public/icon.png`, `docs/` (a short "changing the icon" note).
- No runtime code changes; installers pick the icons up through the existing `bundle.icon` list in `tauri.conf.json`.
