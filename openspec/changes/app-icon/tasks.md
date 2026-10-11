## 1. Artwork and generator

- [x] 1.1 Add `assets/icon/mergeiq-icon.svg` from design board A (geometry and colours as in design.md)
- [x] 1.2 `scripts/icons/build-icons.mjs`: render SVG to 1024 px transparent PNG via Playwright, run `tauri icon`, write the 32 px favicon, prune unneeded mobile outputs; document usage in the script header
- [x] 1.3 Run it; commit the generated `icons/*`, `assets/icon/mergeiq-icon-1024.png` and `public/icon.png`; eyeball 16/32/48/128 px renders

## 2. Guard test

- [x] 2.1 `scripts/icons/icons.test.mjs`: required files exist, PNG sizes, `.icns`/`.ico` containers, stock-icon digest guard; picked up by the existing vitest config (scenarios: Missing file, Wrong size, Guard against the stock icon, Template geometry)

## 3. Bundle and docs

- [x] 3.1 Build the macOS app bundle and verify `Info.plist` icon name and that `Contents/Resources` icon equals the committed `icon.icns`
- [x] 3.2 `docs/changing-the-icon.md` (edit the SVG, run the script, review, commit) and a README contributing pointer
- [ ] 3.3 After the next release: confirm the icon in the Dock, Finder and (Homebrew-installed) `/Applications`; then archive this change
