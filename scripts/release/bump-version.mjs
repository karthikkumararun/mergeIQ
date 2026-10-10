#!/usr/bin/env node
// Sets the app version in tauri.conf.json, apps/desktop/package.json, the workspace
// Cargo.toml and the workspace crates' Cargo.lock entries, in one step.
//
//   node scripts/release/bump-version.mjs 0.2.0 [--no-verify]
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { VERSION_FILES, cargoVersion, jsonVersion } from "./read-versions.mjs";

// https://semver.org/#is-there-a-suggested-regular-expression-regex-to-check-a-semver-string
const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

export const isSemver = (value) => SEMVER.test(value);

/** Replaces the first top-level `"version": "…"` in a JSON file, keeping its formatting. */
export function setJsonVersion(text, file, next) {
  const current = jsonVersion(text, file);
  const pattern = new RegExp(`("version"\\s*:\\s*")${escape(current)}(")`);
  if (!pattern.test(text))
    throw new Error(`${file}: cannot locate the version to replace`);
  return text.replace(pattern, `$1${next}$2`);
}

/** Replaces the `[workspace.package]` version in a Cargo manifest. */
export function setCargoVersion(text, file, next) {
  const current = cargoVersion(text, file);
  const section = /^\[workspace\.package\]\s*$[\s\S]*?(?=^\[|(?![\s\S]))/m;
  const block = text.match(section)[0];
  const updated = block.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${next}$2`);
  void current;
  return text.replace(block, updated);
}

/**
 * Moves workspace crates (`[[package]]` entries with no `source`, i.e. path crates) from
 * version `from` to `to` in a Cargo.lock.
 */
export function setLockVersions(text, from, to) {
  return text.replace(
    /\[\[package\]\]\n(name = "[^"]+"\n)version = "([^"]+)"\n(?!source)/g,
    (all, name, version) =>
      version === from ? `[[package]]\n${name}version = "${to}"\n` : all,
  );
}

function escape(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Computes every file's new content first; nothing is written unless all succeed.
 * Returns the list of files changed.
 */
export function bumpVersion(root, next) {
  if (!isSemver(next)) {
    throw new Error(
      `"${next}" is not a valid semantic version (expected MAJOR.MINOR.PATCH[-prerelease])`,
    );
  }
  const [tauri, pkg, cargo] = VERSION_FILES;
  const read = (file) => readFileSync(join(root, file), "utf8");
  const current = cargoVersion(read(cargo), cargo);
  const edits = [
    [tauri, setJsonVersion(read(tauri), tauri, next)],
    [pkg, setJsonVersion(read(pkg), pkg, next)],
    [cargo, setCargoVersion(read(cargo), cargo, next)],
  ];
  let lock = null;
  try {
    lock = read("Cargo.lock");
  } catch {
    // no lockfile yet: nothing to update
  }
  if (lock !== null)
    edits.push(["Cargo.lock", setLockVersions(lock, current, next)]);
  for (const [file, content] of edits) writeFileSync(join(root, file), content);
  return edits.map(([file]) => file);
}

function main(argv) {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const version = argv.find((a) => !a.startsWith("--"));
  if (!version) {
    console.error("usage: bump-version.mjs <X.Y.Z> [--no-verify]");
    return 2;
  }
  try {
    const files = bumpVersion(root, version);
    console.log(
      `Set version ${version} in:\n${files.map((f) => `  ${f}`).join("\n")}`,
    );
  } catch (error) {
    console.error(error.message);
    return 1;
  }
  if (!argv.includes("--no-verify")) {
    const check = spawnSync(
      "cargo",
      ["metadata", "--format-version", "1", "--offline", "--no-deps"],
      {
        cwd: root,
        stdio: ["ignore", "ignore", "inherit"],
      },
    );
    if (check.status !== 0) {
      console.error("cargo metadata failed after the bump");
      return 1;
    }
  }
  return 0;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = main(process.argv.slice(2));
}
