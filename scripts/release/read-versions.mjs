#!/usr/bin/env node
// Reads the app version from the three files that must agree and checks it against a tag.
//
//   node scripts/release/read-versions.mjs --tag v0.2.0   # gate: exit 1 on any mismatch
//   node scripts/release/read-versions.mjs --consistent   # the three files agree with each other
//   node scripts/release/read-versions.mjs                # print the versions
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Files (relative to the repository root) whose version must match the release tag. */
export const VERSION_FILES = [
  "apps/desktop/src-tauri/tauri.conf.json",
  "apps/desktop/package.json",
  "Cargo.toml",
];

/** The version in a Tauri config or package.json (a top-level `"version"` string). */
export function jsonVersion(text, file) {
  let data;
  try {
    data = JSON.parse(text);
  } catch (error) {
    throw new Error(`${file}: not valid JSON (${error.message})`);
  }
  if (typeof data.version !== "string") {
    throw new Error(`${file}: no top-level "version" string`);
  }
  return data.version;
}

/** The `version` under `[workspace.package]` in a Cargo manifest. */
export function cargoVersion(text, file = "Cargo.toml") {
  const section = text.match(
    /^\[workspace\.package\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m,
  );
  const version = section?.[1].match(/^version\s*=\s*"([^"]+)"/m);
  if (!version)
    throw new Error(`${file}: no version under [workspace.package]`);
  return version[1];
}

/** `{ file: version }` for every file in {@link VERSION_FILES}, read from `root`. */
export function readVersions(root) {
  const read = (file) => readFileSync(join(root, file), "utf8");
  const [tauri, pkg, cargo] = VERSION_FILES;
  return {
    [tauri]: jsonVersion(read(tauri), tauri),
    [pkg]: jsonVersion(read(pkg), pkg),
    [cargo]: cargoVersion(read(cargo), cargo),
  };
}

/** The version a tag names (`v1.2.3` → `1.2.3`), or `null` if it is not a release tag. */
export function tagVersion(tag) {
  const match = tag.match(
    /^v((0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?)$/,
  );
  return match ? match[1] : null;
}

/** Error lines (one per mismatching file, naming file and version); empty when all agree. */
export function mismatches(expected, versions) {
  return Object.entries(versions)
    .filter(([, version]) => version !== expected)
    .map(
      ([file, version]) => `${file} contains ${version}, expected ${expected}`,
    );
}

function main(argv) {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const tagIndex = argv.indexOf("--tag");
  const versions = readVersions(root);
  let expected;
  if (tagIndex >= 0) {
    const tag = argv[tagIndex + 1] ?? "";
    expected = tagVersion(tag);
    if (!expected) {
      console.error(
        `"${tag}" is not a release tag (expected vMAJOR.MINOR.PATCH[-prerelease])`,
      );
      return 1;
    }
  } else if (argv.includes("--consistent")) {
    expected = versions[VERSION_FILES[0]];
  } else {
    console.log(JSON.stringify(versions, null, 2));
    return 0;
  }
  const errors = mismatches(expected, versions);
  if (errors.length > 0) {
    console.error(`Version mismatch (tag/expected version ${expected}):`);
    for (const line of errors) console.error(`  - ${line}`);
    return 1;
  }
  console.log(expected);
  return 0;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = main(process.argv.slice(2));
}
