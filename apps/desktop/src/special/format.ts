import type { SideLabel, SideLabels } from "../ipc/bindings";

/** `18.4 KB`, `38.2 MB`, `512 B`. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unit]}`;
}

/** The contextual name of a side: its branch/ref, or the role when there is none. */
export function sideName(label: SideLabel): string {
  return label.refName ?? label.role;
}

export function leftName(labels: SideLabels): string {
  return sideName(labels.ours);
}

export function rightName(labels: SideLabels): string {
  return sideName(labels.theirs);
}

/** First seven characters of an object id. */
export function shortOid(oid: string | null | undefined): string {
  return oid ? oid.slice(0, 7) : "—";
}

/** `4d7a21…e9c0` from `sha256:4d7a21…e9c0`-style ids. */
export function abbreviateOid(oid: string): string {
  const bare = oid.includes(":") ? oid.slice(oid.indexOf(":") + 1) : oid;
  return bare.length > 12 ? `${bare.slice(0, 6)}…${bare.slice(-4)}` : bare;
}

/** `2d`, `5h`, `just now` from an ISO date, relative to `now`. */
export function ago(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return "";
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "";
  const minutes = Math.max(0, Math.floor((now - then) / 60000));
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 60) return `${days}d`;
  return `${Math.floor(days / 30)}mo`;
}

const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

/** `Sep 30` (UTC) from an ISO date. */
export function shortDate(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}`;
}

/** The last path segment (`vendor/ui-kit` → `ui-kit`). */
export function baseName(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}
