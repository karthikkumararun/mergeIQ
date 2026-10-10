#!/usr/bin/env node
// Renders the Homebrew cask from packaging/homebrew/mergeiq.rb.tmpl using only the release
// version and the .dmg entry of SHA256SUMS.txt.
//
//   node scripts/release/render-cask.mjs --version 0.2.0 --checksums SHA256SUMS.txt \
//     [--signed] [--out Casks/mergeiq.rb]
//
// Exits non-zero (and writes nothing) for a pre-release or invalid version, a missing .dmg
// entry or a digest that is not 64 hex characters. Prints the cask to stdout without --out.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const TEMPLATE_PATH = join(
  here,
  "../../packaging/homebrew/mergeiq.rb.tmpl",
);

/** Only stable releases reach the tap: MAJOR.MINOR.PATCH with no suffix. */
export const STABLE_VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

/** The asset name of the macOS installer for `version`. */
export const dmgName = (version) => `MergeIQ_${version}_universal.dmg`;

/** `{ name: digest }` from `sha256sum` output (`<digest>  <name>`, optional `*` binary mark). */
export function parseChecksums(text) {
  const entries = {};
  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/^(\S+) [ *](.+)$/);
    if (match) entries[match[2]] = match[1];
  }
  return entries;
}

/** The Gatekeeper caveat for an app that is not signed and notarized. */
export const UNSIGNED_CAVEATS = `

  caveats <<~EOS
    MergeIQ is not signed or notarized yet, so macOS Gatekeeper may refuse to open it.
    After installing, clear the quarantine attribute once:

      xattr -dr com.apple.quarantine "#{appdir}/MergeIQ.app"

    Or right-click the app in Applications and choose Open.
  EOS`;

/** Fills the template without validating its inputs (callers validate; see {@link renderCask}). */
export function fillTemplate(template, { version, sha256, signed }) {
  return template
    .replaceAll("{{version}}", version)
    .replaceAll("{{sha256}}", sha256)
    .replace("\n{{caveats}}", signed ? "" : UNSIGNED_CAVEATS);
}

/** Validates the inputs and returns the cask text; throws an Error naming the problem. */
export function renderCask({ template, version, checksums, signed }) {
  if (!STABLE_VERSION.test(version)) {
    throw new Error(
      `refusing to render a cask for "${version}": only stable MAJOR.MINOR.PATCH releases are published to the tap`,
    );
  }
  const asset = dmgName(version);
  const sha256 = parseChecksums(checksums)[asset];
  if (sha256 === undefined) {
    throw new Error(`SHA256SUMS.txt has no entry for ${asset}`);
  }
  if (!/^[0-9a-f]{64}$/.test(sha256)) {
    throw new Error(
      `the digest for ${asset} is not 64 lowercase hex characters: "${sha256}"`,
    );
  }
  return fillTemplate(template, { version, sha256, signed: Boolean(signed) });
}

function main(argv) {
  const arg = (flag) => {
    const i = argv.indexOf(flag);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const version = arg("--version");
  const checksumsFile = arg("--checksums");
  if (!version || !checksumsFile) {
    console.error(
      "usage: render-cask.mjs --version <X.Y.Z> --checksums <file> [--signed] [--out <file>]",
    );
    return 2;
  }
  let cask;
  try {
    cask = renderCask({
      template: readFileSync(TEMPLATE_PATH, "utf8"),
      version,
      checksums: readFileSync(resolve(checksumsFile), "utf8"),
      signed: argv.includes("--signed"),
    });
  } catch (error) {
    console.error(`render-cask: ${error.message}`);
    return 1;
  }
  const out = arg("--out");
  if (out) {
    mkdirSync(dirname(resolve(out)), { recursive: true });
    writeFileSync(resolve(out), cask);
  } else {
    process.stdout.write(cask);
  }
  return 0;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = main(process.argv.slice(2));
}
