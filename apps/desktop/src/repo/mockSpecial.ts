import type {
  ConflictClass,
  GoSumMerge,
  ModifyDeleteView,
  RenameInfo,
  RenamePair,
  StageBlob,
  StageMeta,
  SubmoduleDetails,
} from "../ipc/bindings";

/** Extra data a mocked conflict needs for its special panel. */
export interface MockSpecial {
  /** Per-stage overrides keyed by stage number. */
  stages?: Partial<Record<1 | 2 | 3, Partial<StageMeta>>>;
  images?: Partial<Record<1 | 2 | 3, StageBlob>>;
  modifyDelete?: ModifyDeleteView;
  submodule?: SubmoduleDetails;
  rename?: { pair: RenamePair; renames: RenameInfo[] };
  goSum?: GoSumMerge;
  workingText?: string;
}

export interface SpecialConflict {
  display: string;
  type:
    | "BothModified"
    | "BothAdded"
    | "DeletedByUs"
    | "DeletedByThem"
    | "AddedByUs"
    | "AddedByThem"
    | "BothDeleted";
  /** Fixture that supplies a merge analysis, for text that opens in the editor. */
  fixture: string | null;
  cls: ConflictClass;
  special?: MockSpecial;
}

const MB = 1024 * 1024;

/** A rounded-square logo as an SVG data blob (never inline markup in the UI). */
function logo(
  size: number,
  radius: number,
  fill: string,
  stroke: string,
): StageBlob {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 100 100"><rect x="10" y="10" width="80" height="80" rx="${radius}" fill="${fill}"/><path d="M38 64c4 4 20 4 22-4s-20-8-20-18 16-10 22-6" fill="none" stroke="${stroke}" stroke-width="7" stroke-linecap="round"/></svg>`;
  return { mime: "image/svg+xml", size: svg.length, base64: btoa(svg) };
}

const sha = (prefix: string) => prefix.padEnd(40, "0");
const lfsOid = (head: string, tail: string) =>
  `sha256:${head}${"0".repeat(64 - head.length - tail.length)}${tail}`;

const header = [
  "package com.shop.promo",
  "",
  "import java.time.Instant",
  "import java.time.LocalDateTime",
  "import java.time.ZoneId",
  "import java.time.ZoneOffset",
  "",
  "/**",
  " * A promo code a shopper can apply at checkout.",
  " * Codes are case-insensitive.",
  " */",
  "@Serializable",
  "",
];
const couponBase = [
  ...header,
  "data class Coupon(",
  "    val code: String,",
  "    val expiresAt: LocalDateTime,",
  "    val percent: Int,",
  ")",
  "",
];
const couponKept = [
  ...header,
  "data class Coupon(",
  "    val code: String,",
  "    val expiresAt: Instant,",
  "    val zone: ZoneId = ZoneOffset.UTC,",
  "    val percent: Int,",
  ")",
  "",
];

const goSumRaw: [GoSumMerge["lines"][number]["mark"], string][] = [
  [
    "Unchanged",
    "github.com/google/uuid v1.6.0 h1:NIvaJDMOsjHA8n1jAhLSgzrAzy1Hgr+hNrb57e+94F0=",
  ],
  [
    "Unchanged",
    "github.com/google/uuid v1.6.0/go.mod h1:TIyPZe4MgqvfeYDBFedMoGGpEw/LqOeaOT+nhxU+yHo=",
  ],
  [
    "LeftAdded",
    "github.com/jackc/pgx/v5 v5.7.1 h1:x7SYsPBYDkHDksogeSmZZ5xzThcTgRz++I5E+ePFUcs=",
  ],
  [
    "LeftAdded",
    "github.com/jackc/pgx/v5 v5.7.1/go.mod h1:e7O26IywZZ+naJtWWos6i6fvWK+29etgITqrqHLfoZA=",
  ],
  [
    "Removed",
    "github.com/jackc/pgx/v5 v5.6.0 h1:SWJzexBzPL5jb0GEsrPMLIsi/3jOo7RHlzTjcAeDrPY=",
  ],
  [
    "Unchanged",
    "github.com/prometheus/client_golang v1.20.4 h1:Tgh3Yr67PaOv/uTqloMsCEdeuFTatm5zIq5+qNN23vI=",
  ],
  [
    "RightAdded",
    "github.com/redis/go-redis/v9 v9.6.1 h1:HHDteefn6ZkTtY5fGUE8tj8uy85AHk6zP7CpzIAM0y4=",
  ],
  [
    "RightAdded",
    "github.com/redis/go-redis/v9 v9.6.1/go.mod h1:0C0c6ycQsdpVNQpxb1njEQIqkx5UcsM8FJCQLgE9+RA=",
  ],
  [
    "RightAdded",
    "github.com/stripe/stripe-go/v79 v79.12.0 h1:HQs/kxNEB3gYA7FnkSFkp0kSOeez0fsmCWev6SxftYs=",
  ],
  [
    "RightAdded",
    "github.com/stripe/stripe-go/v79 v79.12.0/go.mod h1:cuH6X0zC8peY6f1AubHwgJ/fJSn2dh5pfiCr6CjyKVU=",
  ],
  [
    "Removed",
    "golang.org/x/net v0.28.0 h1:a9JDOJc5GMUJ0+UDqmLT86WiEy7iWyIhz8gz8E4e5hE=",
  ],
  [
    "Unchanged",
    "golang.org/x/net v0.30.0 h1:AcW1SDZMkb8IpzCdQUaIq2sP4sZ4zw+55h6ynffypl4=",
  ],
];

const commit = (sha7: string, subject: string | null, date: string | null) => ({
  sha: sha(sha7),
  subject,
  date,
});

const renamePair = (
  contentsDiffer: boolean,
): { pair: RenamePair; renames: RenameInfo[] } => {
  const ours: RenameInfo = {
    side: "Ours",
    from: "src/cart/discount.ts",
    to: "src/cart/pricing/discount.ts",
    toPath: "mock:src/cart/pricing/discount.ts",
  };
  const theirs: RenameInfo = {
    side: "Theirs",
    from: "src/cart/discount.ts",
    to: "src/promo/discount.ts",
    toPath: "mock:src/promo/discount.ts",
  };
  return {
    pair: { from: "src/cart/discount.ts", ours, theirs, contentsDiffer },
    renames: [ours, theirs],
  };
};

/** One conflict of every special class, matching the approved boards. */
export function specialConflicts(): SpecialConflict[] {
  return [
    {
      display: "src/main/kotlin/com/shop/promo/Coupon.kt",
      type: "DeletedByThem",
      fixture: null,
      cls: { class: "Text" },
      special: {
        modifyDelete: {
          deletedBy: "Theirs",
          baseText: couponBase.join("\n"),
          survivorText: couponKept.join("\n"),
          hunks: [
            { before: { start: 15, end: 16 }, after: { start: 15, end: 17 } },
          ],
          note: null,
        },
        workingText: couponKept.join("\n"),
      },
    },
    {
      display: "web/assets/logo.png",
      type: "BothModified",
      fixture: null,
      cls: { class: "Binary", isImage: true },
      special: {
        stages: {
          1: { size: Math.round(18.4 * 1024), oid: "3f9a1c2" + "0".repeat(33) },
          2: { size: Math.round(16.9 * 1024), oid: "b72e90d" + "0".repeat(33) },
          3: { size: Math.round(41.2 * 1024), oid: "5d04ae8" + "0".repeat(33) },
        },
        images: {
          1: logo(512, 18, "#3B6FD4", "#fff"),
          2: logo(512, 40, "#3B6FD4", "#fff"),
          3: logo(1024, 18, "#E8A33D", "#16181C"),
        },
      },
    },
    {
      display: "assets/models/scene.bin",
      type: "BothModified",
      fixture: null,
      cls: { class: "Binary", isImage: false },
      special: {
        stages: {
          1: { size: 2 * MB },
          2: { size: Math.round(2.4 * MB) },
          3: { size: Math.round(2.1 * MB) },
        },
      },
    },
    {
      display: "config/current",
      type: "BothModified",
      fixture: null,
      cls: { class: "Symlink" },
      special: {
        stages: {
          1: { mode: "120000", symlinkTarget: "../envs/staging" },
          2: { mode: "120000", symlinkTarget: "../envs/prod-eu" },
          3: { mode: "120000", symlinkTarget: "../envs/prod-us" },
        },
      },
    },
    {
      display: "assets/video/hero.mp4",
      type: "BothModified",
      fixture: null,
      cls: { class: "LfsPointer" },
      special: {
        stages: {
          1: {
            lfs: { oid: lfsOid("4d7a21", "e9c0"), size: Math.round(38.2 * MB) },
          },
          2: {
            lfs: { oid: lfsOid("9b1f03", "27aa"), size: Math.round(36.9 * MB) },
          },
          3: {
            lfs: { oid: lfsOid("c25e88", "b413"), size: Math.round(41.0 * MB) },
          },
        },
      },
    },
    {
      display: "data/exports/orders-2026.csv",
      type: "BothModified",
      fixture: null,
      cls: { class: "Oversized" },
      special: {
        stages: {
          1: { size: Math.round(48.6 * MB) },
          2: { size: Math.round(50.1 * MB) },
          3: { size: Math.round(52.3 * MB) },
        },
      },
    },
    {
      display: "vendor/ui-kit",
      type: "BothModified",
      fixture: null,
      cls: { class: "Submodule" },
      special: {
        submodule: {
          checkedOut: true,
          base: commit("1d9e0b7", "Bump tokens to 3.8", "2026-09-18T10:00:00Z"),
          left: commit(
            "a41f0c2",
            "Fix focus ring on Select",
            "2026-09-30T10:00:00Z",
          ),
          right: commit(
            "7be9d13",
            "Add PromoBadge component",
            "2026-10-04T10:00:00Z",
          ),
          relation: "LeftAncestorOfRight",
        },
      },
    },
    {
      display: "vendor/icons",
      type: "BothModified",
      fixture: null,
      cls: { class: "Submodule" },
      special: {
        submodule: {
          checkedOut: false,
          base: commit("5c4b3a2", null, null),
          left: commit("e7f1d90", null, null),
          right: commit("2a9c6b4", null, null),
          relation: "Unknown",
        },
      },
    },
    {
      display: "vendor/charts",
      type: "BothModified",
      fixture: null,
      cls: { class: "Submodule" },
      special: {
        submodule: {
          checkedOut: true,
          base: commit("11aa22b", "Initial release", "2026-08-01T10:00:00Z"),
          left: commit("33cc44d", "Add pie charts", "2026-09-12T10:00:00Z"),
          right: commit("55ee66f", "Add heat maps", "2026-09-14T10:00:00Z"),
          relation: "Diverged",
        },
      },
    },
    {
      display: "src/cart/discount.ts",
      type: "BothDeleted",
      fixture: null,
      cls: { class: "Text" },
      special: { rename: renamePair(true) },
    },
    {
      display: "src/cart/pricing/discount.ts",
      type: "AddedByUs",
      fixture: null,
      cls: { class: "Text" },
      special: { rename: renamePair(true) },
    },
    {
      display: "src/promo/discount.ts",
      type: "AddedByThem",
      fixture: null,
      cls: { class: "Text" },
      special: { rename: renamePair(true) },
    },
    {
      display: "services/edge/go.sum",
      type: "BothModified",
      fixture: "non-overlapping",
      cls: { class: "Lockfile", kind: "GoSum" },
      special: {
        goSum: {
          lines: goSumRaw.map(([mark, text]) => ({ mark, text })),
          result: "",
        },
      },
    },
    {
      display: "web/pnpm-lock.yaml",
      type: "BothModified",
      fixture: "non-overlapping",
      cls: { class: "Lockfile", kind: "Pnpm" },
    },
    {
      display: "src/app.ts",
      type: "BothModified",
      fixture: "simple-conflict",
      cls: { class: "Text" },
    },
  ];
}

/** Which stages a conflict type has. */
export function stagesOf(type: SpecialConflict["type"]): (1 | 2 | 3)[] {
  switch (type) {
    case "BothModified":
      return [1, 2, 3];
    case "BothAdded":
      return [2, 3];
    case "DeletedByUs":
      return [1, 3];
    case "DeletedByThem":
      return [1, 2];
    case "AddedByUs":
      return [2];
    case "AddedByThem":
      return [3];
    case "BothDeleted":
      return [1];
  }
}

/** The renamePair for `renameChoose`: where the contents differ, a merge is needed. */
export function renameOutcomeFor(contentsDiffer: boolean, chosen: string) {
  return { chosen, needsMerge: contentsDiffer };
}

/** Sample output lines for a mocked lockfile run. */
export const LOCKFILE_OK_LINES = [
  "Lockfile is up to date, resolution step is skipped",
  "Progress: resolved 812, reused 790, downloaded 22, added 0",
];
export const LOCKFILE_FAIL_LINES = [
  " ERR_PNPM_NO_MATCHING_VERSION  No matching version found for @shop/ui-kit@^4.2.0",
  "This error happened while installing a direct dependency of /code/shop-web/web",
];
