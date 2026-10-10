// @vitest-environment node
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  TEMPLATE_PATH,
  fillTemplate,
  parseChecksums,
  renderCask,
} from "./render-cask.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const SCRIPT = join(here, "render-cask.mjs");
const template = readFileSync(TEMPLATE_PATH, "utf8");
const DIGEST = "ab".repeat(32);
const sums = (version, digest = DIGEST) =>
  [
    `${"11".repeat(32)}  MergeIQ_${version}_amd64.deb`,
    `${digest}  MergeIQ_${version}_universal.dmg`,
    `${"22".repeat(32)}  MergeIQ_${version}_x64-setup.exe`,
    "",
  ].join("\n");

const run = (args) =>
  spawnSync(process.execPath, [SCRIPT, ...args], { encoding: "utf8" });

describe("rendering", () => {
  it("fills version, digest and the release URL from the checksums (Render)", () => {
    const cask = renderCask({
      template,
      version: "0.2.0",
      checksums: sums("0.2.0"),
      signed: true,
    });
    expect(cask).toContain('version "0.2.0"');
    expect(cask).toContain(`sha256 "${DIGEST}"`);
    // The URL is built from the version, so it points at the v0.2.0 asset.
    expect(cask).toContain(
      "releases/download/v#{version}/MergeIQ_#{version}_universal.dmg",
    );
    expect(cask).not.toContain("{{");
  });

  it("keeps the install, binary link and zap stanzas", () => {
    const cask = renderCask({
      template,
      version: "0.2.0",
      checksums: sums("0.2.0"),
      signed: true,
    });
    expect(cask).toContain('app "MergeIQ.app"');
    expect(cask).toContain(
      'binary "#{appdir}/MergeIQ.app/Contents/MacOS/mergeiq"',
    );
    expect(cask).toContain("zap trash: [");
    expect(cask).toContain("dev.mergeiq.app");
    expect(cask).toContain("strategy :github_latest");
  });

  it("is macOS-only without a version floor (Homebrew 7 disables `depends_on macos: :catalina`)", () => {
    // The bundle's LSMinimumSystemVersion (10.15) is what keeps older systems from opening it.
    expect(template).toContain("depends_on :macos\n");
    expect(template).not.toContain("depends_on macos:");
  });

  it("adds the Gatekeeper caveat only for an unsigned release (Unsigned caveat)", () => {
    const unsigned = renderCask({
      template,
      version: "0.2.0",
      checksums: sums("0.2.0"),
      signed: false,
    });
    expect(unsigned).toContain("caveats <<~EOS");
    expect(unsigned).toContain(
      'xattr -dr com.apple.quarantine "#{appdir}/MergeIQ.app"',
    );
    expect(unsigned).toMatch(/\]\n\n {2}caveats <<~EOS\n/);
    expect(unsigned.trimEnd().endsWith("EOS\nend")).toBe(true);
  });

  it("has no caveat for a notarized release (Notarized)", () => {
    const signed = renderCask({
      template,
      version: "0.2.0",
      checksums: sums("0.2.0"),
      signed: true,
    });
    expect(signed).not.toContain("caveats");
    expect(signed).not.toContain("xattr");
    // Removing the placeholder leaves no stray blank line before `end`.
    expect(signed).toMatch(/\]\nend\n$/);
  });

  it("parses sha256sum output including binary-mode lines", () => {
    expect(parseChecksums(`${DIGEST}  a.dmg\n${DIGEST} *b.deb\n`)).toEqual({
      "a.dmg": DIGEST,
      "b.deb": DIGEST,
    });
  });
});

describe("refusals", () => {
  it("refuses a pre-release version (Pre-release refused)", () => {
    for (const version of [
      "0.2.0-rc.1",
      "0.1.1-1",
      "0.2",
      "v0.2.0",
      "1.0.0+build",
    ]) {
      expect(() =>
        renderCask({
          template,
          version,
          checksums: sums(version),
          signed: true,
        }),
      ).toThrow(/only stable/);
    }
  });

  it("names the missing asset when the checksums have no .dmg (Missing checksum)", () => {
    const checksums = `${"11".repeat(32)}  MergeIQ_0.2.0_amd64.deb\n`;
    expect(() =>
      renderCask({ template, version: "0.2.0", checksums, signed: true }),
    ).toThrow("MergeIQ_0.2.0_universal.dmg");
  });

  it("does not accept another version's .dmg entry", () => {
    expect(() =>
      renderCask({
        template,
        version: "0.2.0",
        checksums: sums("0.1.0"),
        signed: true,
      }),
    ).toThrow(/no entry for MergeIQ_0.2.0_universal.dmg/);
  });

  it("rejects digests that are not 64 lowercase hex characters", () => {
    for (const bad of [
      "abc",
      "zz".repeat(32),
      "AB".repeat(32),
      "ab".repeat(33),
    ]) {
      expect(() =>
        renderCask({
          template,
          version: "0.2.0",
          checksums: sums("0.2.0", bad),
          signed: true,
        }),
      ).toThrow(/64 lowercase hex/);
    }
  });
});

describe("command line", () => {
  const dir = mkdtempSync(join(tmpdir(), "cask-"));
  const good = join(dir, "SHA256SUMS.txt");
  writeFileSync(good, sums("0.2.0"));

  it("writes the cask to --out", () => {
    const out = join(dir, "Casks", "mergeiq.rb");
    const result = run([
      "--version",
      "0.2.0",
      "--checksums",
      good,
      "--signed",
      "--out",
      out,
    ]);
    expect(result.status, result.stderr).toBe(0);
    expect(readFileSync(out, "utf8")).toContain('version "0.2.0"');
  });

  it("exits non-zero and writes no file for a pre-release (Pre-release refused)", () => {
    const out = join(dir, "pre", "mergeiq.rb");
    const result = run([
      "--version",
      "0.2.0-rc.1",
      "--checksums",
      good,
      "--out",
      out,
    ]);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("only stable");
    expect(existsSync(out)).toBe(false);
  });

  it("exits non-zero naming the missing asset and writes no file (Missing checksum)", () => {
    const empty = join(dir, "empty.txt");
    writeFileSync(empty, `${"11".repeat(32)}  MergeIQ_0.2.0_amd64.deb\n`);
    const out = join(dir, "missing", "mergeiq.rb");
    const result = run([
      "--version",
      "0.2.0",
      "--checksums",
      empty,
      "--out",
      out,
    ]);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("MergeIQ_0.2.0_universal.dmg");
    expect(existsSync(out)).toBe(false);
  });

  it("prints usage and exits 2 without arguments", () => {
    expect(run([]).status).toBe(2);
  });
});

describe("the real v0.1.1-1 release", () => {
  const real = readFileSync(
    join(here, "fixtures/SHA256SUMS-v0.1.1-1.txt"),
    "utf8",
  );

  it("has a parseable .dmg entry in the shipped SHA256SUMS.txt format", () => {
    const dmg = parseChecksums(real)["MergeIQ_0.1.1-1_universal.dmg"];
    expect(dmg).toMatch(/^[0-9a-f]{64}$/);
  });

  it("is refused in production (it is a pre-release)", () => {
    expect(() =>
      renderCask({
        template,
        version: "0.1.1-1",
        checksums: real,
        signed: false,
      }),
    ).toThrow(/only stable/);
  });

  it("can be filled without validation for local audit experiments", () => {
    const cask = fillTemplate(template, {
      version: "0.1.1-1",
      sha256: parseChecksums(real)["MergeIQ_0.1.1-1_universal.dmg"],
      signed: false,
    });
    expect(cask).toContain('version "0.1.1-1"');
    expect(cask).toContain("caveats");
  });
});
