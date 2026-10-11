## ADDED Requirements

### Requirement: Single source of truth
The app icon artwork SHALL live in one committed master file, `assets/icon/mergeiq-icon.svg`, and every raster or platform icon file under `apps/desktop/src-tauri/icons/` and the web favicon SHALL be generated from it by `scripts/icons/build-icons.mjs`. Generated files SHALL be committed so that building the app needs no icon tooling.

#### Scenario: Regenerate
- **WHEN** `node scripts/icons/build-icons.mjs` runs on a clean checkout
- **THEN** `icon.icns`, `icon.ico`, `icon.png`, `32x32.png`, `128x128.png`, `128x128@2x.png`, the Windows Store logos and `apps/desktop/public/icon.png` are written from the SVG and `git status` shows no unrelated changes

#### Scenario: No second source
- **WHEN** the SVG is edited and the script is rerun
- **THEN** every generated file changes consistently and no icon file is edited by hand

### Requirement: Brand design
The icon SHALL show two amber (`#e8a33d` family) branches merging into one stem on a dark (`#14161a`–`#2a2e36`) rounded square, with the two source nodes as dark discs with amber rings and the result node as a filled amber disc. The artwork SHALL follow the macOS icon template: an 824 px rounded square (corner radius 188) centred in a 1024 px transparent canvas, with a soft drop shadow inside the margin.

#### Scenario: Template geometry
- **WHEN** `icon.png` is inspected
- **THEN** it is 1024 px square at its source, the opaque rounded square spans 824 px centred, and the corners and margin are transparent

#### Scenario: Matches the UI mark
- **WHEN** the icon and the app header mark are compared
- **THEN** both show the same two-branches-into-one form in the same amber

### Requirement: Legible at small sizes
The icon SHALL remain recognizable at 16, 32 and 48 px: the three nodes and the merged stem stay distinct and no detail relies on strokes thinner than 1 px at 16 px.

#### Scenario: Small sizes
- **WHEN** the icon is rendered at 16, 32 and 48 px on light and dark backgrounds
- **THEN** the amber branches are visible against the dark square and the square reads against both backgrounds

### Requirement: Every platform bundle uses it
`tauri.conf.json`'s `bundle.icon` list SHALL reference only generated files from this change, and the built macOS app, Windows installers and Linux packages SHALL show the MergeIQ icon, not the Tauri default.

#### Scenario: macOS bundle
- **WHEN** the macOS app bundle is built
- **THEN** its `Info.plist` names an icon file that exists in `Contents/Resources` and equals the repository's `icon.icns`

#### Scenario: Guard against the stock icon
- **WHEN** any file under `apps/desktop/src-tauri/icons/` matches the stock Tauri icon digest
- **THEN** the guard test fails with a message naming the file

### Requirement: Generated files are complete and correctly sized
The guard test SHALL verify that the committed icon files exist and have the expected pixel sizes (`32x32.png` 32 px, `128x128.png` 128 px, `128x128@2x.png` 256 px, `icon.png` 512 px or larger and square) and that `icon.icns` and `icon.ico` are non-empty files of the right container type.

#### Scenario: Missing file
- **WHEN** `icon.ico` is deleted
- **THEN** the guard test fails naming `icon.ico`

#### Scenario: Wrong size
- **WHEN** `128x128.png` is replaced by a 64 px image
- **THEN** the guard test fails naming the file and the expected size
