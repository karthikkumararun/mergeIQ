/** A row of the preview: unchanged context, a removed line or an added line. */
export interface DiffRow {
  kind: "ctx" | "del" | "add";
  text: string;
  /** Character range to emphasise (changed tokens), if any. */
  emphasis?: [number, number];
}

const MAX_CELLS = 250_000;

/** Longest-common-subsequence line diff (small inputs; falls back to replace-all). */
function lcsOps(a: string[], b: string[]): ("ctx" | "del" | "add")[] {
  const n = a.length;
  const m = b.length;
  if (n * m > MAX_CELLS) {
    return [...a.map(() => "del" as const), ...b.map(() => "add" as const)];
  }
  const dp: number[][] = Array.from({ length: n + 1 }, () =>
    new Array<number>(m + 1).fill(0),
  );
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      dp[i][j] =
        a[i] === b[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const ops: ("ctx" | "del" | "add")[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      ops.push("ctx");
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push("del");
      i++;
    } else {
      ops.push("add");
      j++;
    }
  }
  while (i < n) {
    ops.push("del");
    i++;
  }
  while (j < m) {
    ops.push("add");
    j++;
  }
  return ops;
}

/** Range of `s` without its leading/trailing whitespace, or `undefined` if blank. */
export function trimmed(s: string): [number, number] | undefined {
  const start = s.length - s.trimStart().length;
  const end = s.trimEnd().length;
  return end > start ? [start, end] : undefined;
}

/** The differing middle of two lines (common prefix and suffix removed). */
export function middle(
  a: string,
  b: string,
): { a?: [number, number]; b?: [number, number] } {
  let p = 0;
  while (p < a.length && p < b.length && a[p] === b[p]) p++;
  let s = 0;
  while (
    s < a.length - p &&
    s < b.length - p &&
    a[a.length - 1 - s] === b[b.length - 1 - s]
  )
    s++;
  const range = (t: string): [number, number] | undefined =>
    t.length - s > p ? [p, t.length - s] : undefined;
  return { a: range(a), b: range(b) };
}

/**
 * Line diff of `before` against `after` for the preview. Changed lines are emphasised: a
 * removed line paired with an added line shows only the differing tokens; unpaired lines are
 * emphasised in full (without indentation).
 */
export function diffRows(before: string[], after: string[]): DiffRow[] {
  const ops = lcsOps(before, after);
  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  let k = 0;
  while (k < ops.length) {
    if (ops[k] === "ctx") {
      rows.push({ kind: "ctx", text: before[i] });
      i++;
      j++;
      k++;
      continue;
    }
    const dels: string[] = [];
    const adds: string[] = [];
    while (k < ops.length && ops[k] !== "ctx") {
      if (ops[k] === "del") dels.push(before[i++]);
      else adds.push(after[j++]);
      k++;
    }
    const pairs = Math.min(dels.length, adds.length);
    const delRows: DiffRow[] = dels.map((text, d) => ({
      kind: "del",
      text,
      emphasis: d < pairs ? middle(text, adds[d]).a : trimmed(text),
    }));
    const addRows: DiffRow[] = adds.map((text, a) => ({
      kind: "add",
      text,
      emphasis: a < pairs ? middle(dels[a], text).b : trimmed(text),
    }));
    rows.push(...delRows, ...addRows);
  }
  return rows;
}
