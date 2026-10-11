// @vitest-environment node
// Guards the app icon (openspec app-icon): the generated files exist, are the right size, are not
// the stock Tauri icon, and the 1024 px source follows the macOS template. Scenario names are the
// test names.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const icons = join(root, "apps", "desktop", "src-tauri", "icons");
const master = join(root, "assets", "icon", "mergeiq-icon-1024.png");

/** SHA-256 of the placeholder icons the app shipped with before the real one. */
const STOCK = {
  "icon.png":
    "b5d93c8ec365c08b11bd006e46a46e227c681d30bb295af3d017573bfc752a83",
  "icon.icns":
    "3dc10493b7de48a61de58f768f8a5708d3a44a068c148cedf0502b9b9b71ba5d",
  "icon.ico":
    "e38ca88e1d5490f3dcbc3c3fa525f7fcb7b80fff3cb2f3a4eb1b2d018c0915c1",
  "128x128.png":
    "1f3689f6374b0553996fdc99743799216703835070145d7f0e6ec11e6280139e",
  "32x32.png":
    "d151f11e325f7502de0c739a2e51697aa569fd4701ae6e11fae1a3b4c7d5f157",
};

const REQUIRED_PNG = {
  "32x32.png": 32,
  "128x128.png": 128,
  "128x128@2x.png": 256,
};

const sha256 = (file) =>
  createHash("sha256").update(readFileSync(file)).digest("hex");

function pngSize(file) {
  const b = readFileSync(file);
  expect(b.subarray(1, 4).toString(), `${file} is not a PNG`).toBe("PNG");
  return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) };
}

/** RGBA pixel reader for the 8-bit, non-interlaced PNGs this repo generates. */
function rgbaReader(file) {
  const b = readFileSync(file);
  const width = b.readUInt32BE(16);
  const height = b.readUInt32BE(20);
  expect(b[24], "8-bit").toBe(8);
  expect(b[25], "RGBA").toBe(6);
  expect(b[28], "not interlaced").toBe(0);
  const idat = [];
  for (let at = 8; at < b.length;) {
    const len = b.readUInt32BE(at);
    if (b.subarray(at + 4, at + 8).toString() === "IDAT") {
      idat.push(b.subarray(at + 8, at + 8 + len));
    }
    at += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * 4;
  const px = Buffer.alloc(height * stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= 4 ? px[y * stride + x - 4] : 0;
      const up = y > 0 ? px[(y - 1) * stride + x] : 0;
      const c = x >= 4 && y > 0 ? px[(y - 1) * stride + x - 4] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += up;
      else if (filter === 3) v += (a + up) >> 1;
      else if (filter === 4) {
        const p = a + up - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - up);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? up : c;
      }
      px[y * stride + x] = v & 255;
    }
  }
  return { width, height, alpha: (x, y) => px[y * stride + x * 4 + 3] };
}

describe("app icon", () => {
  it("has every required icon file (Missing file)", () => {
    const names = [
      ...Object.keys(REQUIRED_PNG),
      "icon.png",
      "icon.icns",
      "icon.ico",
    ];
    for (const name of names) {
      expect(existsSync(join(icons, name)), `${name} is missing`).toBe(true);
    }
    expect(existsSync(master), "assets/icon/mergeiq-icon-1024.png").toBe(true);
    expect(existsSync(join(root, "assets", "icon", "mergeiq-icon.svg"))).toBe(
      true,
    );
    expect(
      existsSync(join(root, "apps", "desktop", "public", "icon.png")),
    ).toBe(true);
  });

  it("has the expected pixel sizes (Wrong size)", () => {
    for (const [name, size] of Object.entries(REQUIRED_PNG)) {
      expect(pngSize(join(icons, name)), `${name} must be ${size} px`).toEqual({
        width: size,
        height: size,
      });
    }
    const main = pngSize(join(icons, "icon.png"));
    expect(main.width, "icon.png is square").toBe(main.height);
    expect(main.width, "icon.png >= 512 px").toBeGreaterThanOrEqual(512);
  });

  it("has icns and ico containers", () => {
    const icns = readFileSync(join(icons, "icon.icns"));
    expect(icns.subarray(0, 4).toString()).toBe("icns");
    expect(icns.length).toBeGreaterThan(10_000);
    const ico = readFileSync(join(icons, "icon.ico"));
    expect([...ico.subarray(0, 4)]).toEqual([0, 0, 1, 0]);
    expect(statSync(join(icons, "icon.ico")).size).toBeGreaterThan(1_000);
  });

  it("is not the stock Tauri icon (Guard against the stock icon)", () => {
    for (const [name, digest] of Object.entries(STOCK)) {
      const file = join(icons, name);
      if (existsSync(file)) {
        expect(sha256(file), `${name} is the stock Tauri icon`).not.toBe(
          digest,
        );
      }
    }
  });

  it("follows the macOS template: 824 px rounded square centred in 1024 px (Template geometry)", () => {
    const img = rgbaReader(master);
    expect(img.width).toBe(1024);
    expect(img.height).toBe(1024);
    // Transparent corners and margin, opaque centre and square body.
    expect(img.alpha(0, 0)).toBe(0);
    expect(img.alpha(1023, 1023)).toBe(0);
    expect(img.alpha(512, 40)).toBe(0);
    expect(img.alpha(512, 512)).toBe(255);
    expect(img.alpha(110, 512)).toBe(255);
    expect(img.alpha(913, 512)).toBe(255);
    // The rounded corner (radius 188) is cut: the square's own corner is transparent.
    expect(img.alpha(104, 104)).toBeLessThan(255);
  });
});
