// @vitest-environment node
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { assetErrors } from "./check-assets.mjs";
import { renderNotes, unsignedNotice } from "./render-notes.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const V = "0.2.0";
const COMPLETE = [
  `MergeIQ_${V}_universal.dmg`,
  `MergeIQ_${V}_x64_en-US.msi`,
  `MergeIQ_${V}_x64-setup.exe`,
  `MergeIQ_${V}_amd64.AppImage`,
  `MergeIQ_${V}_amd64.deb`,
];

describe("asset set", () => {
  it("accepts exactly one of each installer, each naming the version (Complete asset set)", () => {
    expect(assetErrors(COMPLETE, V)).toEqual([]);
    // Non-installer files (the checksum file) are not part of the installer set.
    expect(assetErrors([...COMPLETE, "SHA256SUMS.txt"], V)).toEqual([]);
  });

  it("reports missing and duplicate installers", () => {
    const errors = assetErrors(COMPLETE.slice(0, 4), V);
    expect(errors).toEqual(["missing a .deb installer"]);
    const dup = assetErrors([...COMPLETE, `MergeIQ_${V}_arm64.deb`], V);
    expect(dup[0]).toMatch(/expected one \.deb installer, found 2/);
  });

  it("reports names without the version", () => {
    const names = COMPLETE.map((n) => n.replace(V, "0.1.0"));
    expect(assetErrors(names, V)).toHaveLength(5);
    expect(assetErrors(names, V)[0]).toContain(
      "does not contain the version 0.2.0",
    );
  });

  it("the CLI exits 1 listing problems and 0 for a complete directory", () => {
    const dir = mkdtempSync(join(tmpdir(), "assets-"));
    for (const name of COMPLETE.slice(0, 2))
      writeFileSync(join(dir, name), "x");
    const bad = spawnSync(
      process.execPath,
      [join(here, "check-assets.mjs"), V, dir],
      { encoding: "utf8" },
    );
    expect(bad.status).toBe(1);
    expect(bad.stderr).toContain("missing a .exe installer");
    for (const name of COMPLETE) writeFileSync(join(dir, name), "x");
    const ok = spawnSync(
      process.execPath,
      [join(here, "check-assets.mjs"), V, dir],
      { encoding: "utf8" },
    );
    expect(ok.status, ok.stderr).toBe(0);
  });
});

describe("release notes", () => {
  const template = readFileSync(join(here, "notes-template.md"), "utf8");
  const render = (extra = {}) =>
    renderNotes({
      generated: "## What's Changed\n* Fix a thing in #1",
      template,
      repo: "owner/mergeiq",
      tag: `v${V}`,
      assets: [...COMPLETE, "SHA256SUMS.txt"],
      ...extra,
    });

  it("puts generated notes first, then an Install section linking every installer", () => {
    const notes = render();
    expect(notes.startsWith("## What's Changed")).toBe(true);
    expect(notes).toContain("## Install");
    for (const name of COMPLETE) {
      expect(notes).toContain(
        `https://github.com/owner/mergeiq/releases/download/v${V}/${encodeURIComponent(name)}`,
      );
    }
    expect(notes).toContain("sha256sum --check SHA256SUMS.txt");
    expect(notes).not.toContain("{{");
  });

  it("says nothing about signing when everything is signed", () => {
    expect(render()).not.toContain("not code-signed");
  });

  it("names unsigned macOS and the Gatekeeper override (Unsigned build notice)", () => {
    const notes = render({ unsignedMacos: true });
    expect(notes).toContain("### Unsigned installers");
    expect(notes).toContain("**macOS**");
    expect(notes).toContain("Gatekeeper");
    expect(notes).toContain("xattr -dr com.apple.quarantine");
    expect(notes).not.toContain("**Windows** — not signed");
  });

  it("names every unsigned platform", () => {
    const text = unsignedNotice({ macos: true, windows: true });
    expect(text).toContain("**macOS**");
    expect(text).toContain("**Windows**");
    expect(unsignedNotice({ macos: false, windows: false })).toBe("");
  });

  it("the CLI renders notes from a directory of assets", () => {
    const dir = mkdtempSync(join(tmpdir(), "notes-"));
    for (const name of COMPLETE) writeFileSync(join(dir, name), "x");
    const gen = join(dir, "..", `gen-${Date.now()}.md`);
    writeFileSync(gen, "## What's Changed\n");
    const result = spawnSync(
      process.execPath,
      [
        join(here, "render-notes.mjs"),
        "--tag",
        `v${V}`,
        "--repo",
        "o/r",
        "--assets",
        dir,
        "--generated",
        gen,
        "--unsigned-macos",
      ],
      { encoding: "utf8" },
    );
    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout).toContain("Unsigned installers");
  });
});
