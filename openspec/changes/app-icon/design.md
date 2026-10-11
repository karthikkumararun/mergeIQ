## Context

The bootstrap used Tauri's placeholder icons. The UI already has a header mark (two lines merging, amber on dark). The approved Claude Design canvas explored three directions (A · Merge Y, B · Three-way, C · amber with spark) and a size check; the user chose A. Tauri generates every platform file from one square PNG with `tauri icon`.

## Goals / Non-Goals

**Goals:**
- One reproducible source and generator; committed outputs.
- Looks right in the macOS Dock (template geometry) and stays legible at 16 px.

**Non-Goals:**
- Separate Windows/Linux artwork (same icon everywhere for now).
- Animated or themed (light/dark) icon variants; macOS 26 layered `.icon` format: later if wanted.
- Changing the header mark inside the UI.

## Decisions

**Artwork.** Master SVG in `assets/icon/mergeiq-icon.svg`, 1024 viewBox: 824 px rounded square (rx 188) with vertical dark gradient `#2a2e36 → #14161a`, 4 px inner highlight at 8% white, drop shadow (dy 18, blur 22, 45%) inside the 100 px margin. Mark: stroke 72 round caps in amber gradient `#f6c15d → #e8a33d`; source nodes r 58 dark fill with 30 px `#f0b252` ring at (342,322) and (682,322); result node r 58 filled `#f0b252` at (512,742); curves `M342 322 C342 462 512 468 512 590`, mirrored, and stem `M512 590 L512 726`. Colours and geometry are exactly the approved canvas board A.

**One artwork on all platforms.** The macOS margin and shadow are baked into the PNG, so on Windows and Linux the square is slightly inset. Accepted: one source beats three; revisit if it looks small on Windows.

**Generation.** `scripts/icons/build-icons.mjs` renders the SVG to a 1024 px transparent PNG with Playwright's Chromium (already a dev dependency, so no new tool), writes `assets/icon/mergeiq-icon-1024.png` (committed, the generator input), then runs `pnpm tauri icon` on it for `apps/desktop/src-tauri/icons/`, and writes a 32 px favicon to `apps/desktop/public/icon.png`. Rendering through a browser keeps gradients, filters and anti-aliasing identical to what designers saw.

**Guard test.** `scripts/icons/icons.test.mjs` (vitest, same pattern as `scripts/release`): decodes PNG headers for dimensions, checks `.icns` magic (`icns`) and `.ico` header, and fails on the stock icon SHA-256 recorded in the test. It deliberately does not render pixels, so it is fast and platform independent.

**Why not hand-draw per-size icons.** The three nodes and stem survive down to 16 px in the approved size board; optical sizes can be added later if a size looks muddy.

## Implementation notes (resolved during apply)

- **The approved canvas had a rendering bug the build exposed.** In the Claude Design boards the straight stem used an `objectBoundingBox` gradient, which paints nothing on a zero-width line, so the stem was invisible and the result node floated below the branches. The master SVG uses `gradientUnits="userSpaceOnUse"` (y 290 to 770), the boards were fixed the same way, and `docs/changing-the-icon.md` records the pitfall.
- **Generated through Chromium, verified in the bundle.** The rendered PNG, favicon and `tauri icon` outputs are committed; a debug `tauri build --bundles app` produced a `MergeIQ.app` whose `CFBundleIconFile` is `icon.icns` and whose icon file is byte-identical to the repository's. The extracted 16/32/128 px renders were checked on light and dark backgrounds: the Y is recognizable at 16 px (rings blur), clear at 32.
- **Guard test decodes PNG alpha** with Node's `zlib` (no new dependency) to check the template margin and rounded corners; mutation checks (missing `icon.ico`, a 64 px `128x128.png`) fail as specified.
- `tauri icon` also writes Android/iOS outputs and a `64x64.png`; the script deletes `android/` and `ios/`, and keeps `64x64.png` (unused but harmless).

## Risks / Trade-offs

- Chromium rendering can differ slightly between versions; output is committed, and the script is only run when the artwork changes.
- The baked shadow is part of the PNG; on Windows 10/11 it looks like a soft glow. Revisit with a no-shadow variant for `.ico` if needed.
- `tauri icon` overwrites the whole directory: files not in `bundle.icon` (Store logos) are regenerated too; the script removes mobile outputs it does not need.

## Open Questions

- Do we want a Dock-only dark/light variant on macOS 26 (layered icon)? Not now.
