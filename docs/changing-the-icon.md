# Changing the app icon

The icon has one source of truth: [`assets/icon/mergeiq-icon.svg`](../assets/icon/mergeiq-icon.svg), a 1024 px
canvas following the macOS icon template (an 824 px rounded square, corner radius 188, centred with a soft
shadow in the margin). Everything else is generated and committed, so building the app needs none of this.

1. Edit the SVG. Keep the template geometry; the guard test checks it.
2. Regenerate everything:

   ```sh
   pnpm install
   node scripts/icons/build-icons.mjs
   ```

   This renders `assets/icon/mergeiq-icon-1024.png` with Playwright's Chromium (a dev dependency of the
   desktop app; run `pnpm --filter desktop exec playwright install chromium` once if it is missing), runs
   `tauri icon` to rewrite `apps/desktop/src-tauri/icons/` (`icon.icns`, `icon.ico`, PNG sizes, Windows
   Store logos) and writes the 32 px favicon `apps/desktop/public/icon.png`.

3. Look at the result at 16, 32, 128 and 1024 px on a light and a dark background.
4. Run `pnpm --filter desktop test`; `scripts/icons/icons.test.mjs` fails on missing files, wrong sizes, the
   old stock Tauri icon, or a broken template.
5. Commit the SVG and every generated file together.

## Gotcha: gradients on straight lines

An SVG gradient defined in the default `objectBoundingBox` units paints nothing on a perfectly vertical or
horizontal line, because the line's bounding box has zero width or height. The amber gradient therefore uses
`gradientUnits="userSpaceOnUse"` with fixed coordinates. Do the same for any new gradient stroke.

## Checking a built app on macOS

```sh
pnpm --filter desktop exec tauri build --debug --bundles app
A=target/debug/bundle/macos/MergeIQ.app
/usr/libexec/PlistBuddy -c 'Print :CFBundleIconFile' "$A/Contents/Info.plist"
cmp "$A/Contents/Resources/icon.icns" apps/desktop/src-tauri/icons/icon.icns && echo identical
```
