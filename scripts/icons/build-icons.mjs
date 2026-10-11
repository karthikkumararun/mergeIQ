#!/usr/bin/env node
// Regenerates every app icon from the one master, assets/icon/mergeiq-icon.svg:
//   1. renders the SVG to a 1024 px transparent PNG (assets/icon/mergeiq-icon-1024.png) with
//      Playwright's Chromium, so gradients and the shadow look exactly as designed;
//   2. runs `tauri icon` on it for apps/desktop/src-tauri/icons/ (icns, ico, PNG sizes, Store logos);
//   3. writes the 32 px web favicon, apps/desktop/public/icon.png (the square only, no margin).
// Generated files are committed, so building the app needs none of this. Run it only when the
// artwork changes:  node scripts/icons/build-icons.mjs   (see docs/changing-the-icon.md)

import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const desktop = join(root, "apps", "desktop");
const svgPath = join(root, "assets", "icon", "mergeiq-icon.svg");
const pngPath = join(root, "assets", "icon", "mergeiq-icon-1024.png");
const iconsDir = join(desktop, "src-tauri", "icons");
const faviconPath = join(desktop, "public", "icon.png");

// Playwright is a dev dependency of the desktop app; resolve it from there.
const require = createRequire(join(desktop, "package.json"));
const { chromium } = require("@playwright/test");

/** The SVG with its drawn size and viewBox replaced (the master keeps the 1024 template). */
export function resized(svg, size, viewBox = "0 0 1024 1024") {
  return svg
    .replace(/width="1024" height="1024"/, `width="${size}" height="${size}"`)
    .replace(/viewBox="[^"]*"/, `viewBox="${viewBox}"`);
}

async function render(browser, svg, size) {
  const page = await browser.newPage({
    viewport: { width: size, height: size },
  });
  await page.setContent(
    `<!doctype html><body style="margin:0;background:transparent">${svg}</body>`,
  );
  const png = await page.screenshot({ omitBackground: true, type: "png" });
  await page.close();
  return png;
}

async function main() {
  const svg = readFileSync(svgPath, "utf8");
  const browser = await chromium.launch();
  try {
    writeFileSync(pngPath, await render(browser, resized(svg, 1024), 1024));
    mkdirSync(dirname(faviconPath), { recursive: true });
    writeFileSync(
      faviconPath,
      await render(browser, resized(svg, 32, "100 100 824 824"), 32),
    );
  } finally {
    await browser.close();
  }

  const run = spawnSync(
    "pnpm",
    ["exec", "tauri", "icon", pngPath, "--output", iconsDir],
    { cwd: desktop, stdio: "inherit" },
  );
  if (run.status !== 0) {
    console.error("tauri icon failed");
    process.exit(run.status ?? 1);
  }
  // Mobile outputs are not used by this desktop app.
  for (const dir of ["android", "ios"]) {
    rmSync(join(iconsDir, dir), { recursive: true, force: true });
  }
  console.log(`Icons regenerated from ${svgPath}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
