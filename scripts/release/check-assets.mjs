#!/usr/bin/env node
// Checks that a set of release asset names is exactly the expected installer set.
//
//   node scripts/release/check-assets.mjs <version> <directory>
import { readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Installer kinds a release must contain exactly one of. */
export const INSTALLER_EXTENSIONS = [
  ".dmg",
  ".msi",
  ".exe",
  ".AppImage",
  ".deb",
];

/** Names that are installers (by extension), in the order given. */
export const installers = (names) =>
  names.filter((name) =>
    INSTALLER_EXTENSIONS.some((ext) => name.endsWith(ext)),
  );

/** Problems with `names` for `version`; empty when the asset set is complete. */
export function assetErrors(names, version) {
  const errors = [];
  for (const ext of INSTALLER_EXTENSIONS) {
    const found = names.filter((name) => name.endsWith(ext));
    if (found.length === 0) errors.push(`missing a ${ext} installer`);
    if (found.length > 1)
      errors.push(
        `expected one ${ext} installer, found ${found.length}: ${found.join(", ")}`,
      );
  }
  for (const name of installers(names)) {
    if (!name.includes(version))
      errors.push(`${name} does not contain the version ${version}`);
    if (!name.startsWith("MergeIQ"))
      errors.push(`${name} does not start with the product name`);
  }
  return errors;
}

function main(argv) {
  const [version, directory] = argv;
  if (!version || !directory) {
    console.error("usage: check-assets.mjs <version> <directory>");
    return 2;
  }
  const names = readdirSync(resolve(directory));
  const errors = assetErrors(names, version);
  if (errors.length > 0) {
    console.error(`Release assets in ${directory} are not the expected set:`);
    for (const error of errors) console.error(`  - ${error}`);
    return 1;
  }
  console.log(installers(names).join("\n"));
  return 0;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = main(process.argv.slice(2));
}
