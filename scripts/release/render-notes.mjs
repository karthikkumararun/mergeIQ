#!/usr/bin/env node
// Builds the release notes: GitHub's generated notes, then the Install section from
// notes-template.md (with an unsigned-build notice when needed).
//
//   node scripts/release/render-notes.mjs --tag v0.2.0 --repo owner/name --assets <dir> \
//     --generated generated.md [--unsigned-macos] [--unsigned-windows]
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { installers } from "./check-assets.mjs";

const here = dirname(fileURLToPath(import.meta.url));

const PLATFORMS = [
  { name: "macOS (Apple silicon and Intel)", match: (n) => n.endsWith(".dmg") },
  {
    name: "Windows (x64)",
    match: (n) => n.endsWith(".msi") || n.endsWith(".exe"),
  },
  {
    name: "Linux (x86_64)",
    match: (n) => n.endsWith(".AppImage") || n.endsWith(".deb"),
  },
];

const kind = (name) => (name.match(/\.[A-Za-z]+$/)?.[0] ?? "").slice(1);

/** One markdown table row per platform, linking each installer. */
export function installTable(assets, url) {
  const rows = PLATFORMS.map((platform) => {
    const links = assets
      .filter(platform.match)
      .map((name) => `[${kind(name)}](${url(name)})`);
    return `| ${platform.name} | ${links.join(" · ") || "—"} |`;
  });
  return ["| Platform | Download |", "| --- | --- |", ...rows].join("\n");
}

/** The notice for platforms whose installers are not signed; empty when all are signed. */
export function unsignedNotice({ macos, windows }) {
  const lines = [];
  if (macos) {
    lines.push(
      "- **macOS** — not signed or notarized. macOS Gatekeeper will refuse to open it at first. Open the app with right-click › **Open**, or run `xattr -dr com.apple.quarantine /Applications/MergeIQ.app` after dragging it to Applications.",
    );
  }
  if (windows) {
    lines.push(
      "- **Windows** — not signed. SmartScreen shows “Windows protected your PC”: choose **More info › Run anyway**.",
    );
  }
  if (lines.length === 0) return "";
  return [
    "",
    "### Unsigned installers",
    "",
    "These installers are not code-signed:",
    "",
    ...lines,
    "",
  ].join("\n");
}

/** Generated notes, then the filled-in template. */
export function renderNotes({
  generated,
  template,
  repo,
  tag,
  assets,
  unsignedMacos,
  unsignedWindows,
}) {
  const url = (name) =>
    `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(name)}`;
  const install = template
    .replaceAll("{{install_table}}", installTable(installers(assets), url))
    .replaceAll(
      "{{unsigned_notice}}",
      unsignedNotice({ macos: unsignedMacos, windows: unsignedWindows }),
    )
    .replaceAll("{{repo}}", repo)
    .replaceAll("{{tag}}", tag);
  return `${generated.trim()}\n\n${install.trim()}\n`;
}

function main(argv) {
  const arg = (flag) => {
    const i = argv.indexOf(flag);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const [tag, repo, assetsDir] = [arg("--tag"), arg("--repo"), arg("--assets")];
  if (!tag || !repo || !assetsDir) {
    console.error(
      "usage: render-notes.mjs --tag <tag> --repo <owner/name> --assets <dir> [--generated <file>]",
    );
    return 2;
  }
  const generatedFile = arg("--generated");
  process.stdout.write(
    renderNotes({
      generated: generatedFile ? readFileSync(generatedFile, "utf8") : "",
      template: readFileSync(join(here, "notes-template.md"), "utf8"),
      repo,
      tag,
      assets: readdirSync(resolve(assetsDir)),
      unsignedMacos: argv.includes("--unsigned-macos"),
      unsignedWindows: argv.includes("--unsigned-windows"),
    }),
  );
  return 0;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  process.exitCode = main(process.argv.slice(2));
}
