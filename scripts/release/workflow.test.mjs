// @vitest-environment node
// Guards .github/workflows/release.yml against the v0.2.3 publish failure: `mkdir assets` ran in the
// checkout, where a tracked `assets/` folder (the icon master) already existed. A plain relative
// `mkdir` in a workflow step must never name something that exists in the repository.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const workflow = readFileSync(
  join(root, ".github", "workflows", "release.yml"),
  "utf8",
);

/** Literal (no `$var`) targets of every `mkdir` in the workflow. */
export function literalMkdirTargets(text) {
  const targets = [];
  for (const line of text.split("\n")) {
    const match = line.match(/\bmkdir\b((?:\s+-\w+)*)\s+(\S+)/);
    if (!match) continue;
    const target = match[2].replace(/^["']|["']$/g, "");
    if (!target.includes("$") && !target.includes("{{")) targets.push(target);
  }
  return targets;
}

describe("release workflow", () => {
  it("finds literal mkdir targets", () => {
    expect(
      literalMkdirTargets(
        '  mkdir assets\n  mkdir -p "$tap/Casks"\n  mkdir -p out/x',
      ),
    ).toEqual(["assets", "out/x"]);
  });

  it("never creates a directory that already exists in the repository (publish assets dir)", () => {
    for (const target of literalMkdirTargets(workflow)) {
      expect(
        existsSync(join(root, target)),
        `release.yml runs \`mkdir ${target}\`, but ${target}/ exists in the repo, so the step fails; use $RUNNER_TEMP`,
      ).toBe(false);
    }
  });
});
