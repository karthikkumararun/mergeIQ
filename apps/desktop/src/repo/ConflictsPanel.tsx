import { useMemo } from "react";
import type { ConflictEntry } from "../ipc/bindings";
import { classBadge, sideChanges, splitPath, typeLabel } from "./describe";
import { ResolvedSection } from "./ResolvedSection";
import type { AcceptSide, RepoStatus } from "./repoApi";
import type { ResolvedItem, ViewMode } from "./repoStore";
import { VirtualList } from "./VirtualList";
import { visibleRows, type Row } from "./visibleRows";
import styles from "./ConflictsPanel.module.css";

const ROW_HEIGHT = 56;

interface Props {
  status: RepoStatus;
  filter: string;
  view: ViewMode;
  selection: string[];
  activePath: string | null;
  resolved: ResolvedItem[];
  onFilter: (filter: string) => void;
  onView: (view: ViewMode) => void;
  onToggle: (path: string) => void;
  onSelectAll: (paths: string[]) => void;
  onClearSelection: () => void;
  onOpen: (path: string) => void;
  onAccept: (paths: string[], side: AcceptSide) => void;
  onReopen: (path: string) => void;
}

/** Left panel: filter, selection actions, the virtualized conflict list, resolved log. */
export function ConflictsPanel({
  status,
  filter,
  view,
  selection,
  activePath,
  resolved,
  onFilter,
  onView,
  onToggle,
  onSelectAll,
  onClearSelection,
  onOpen,
  onAccept,
  onReopen,
}: Props) {
  const rows = useMemo(
    () => visibleRows(status.conflicts, filter, view),
    [status.conflicts, filter, view],
  );
  const shown = rows.flatMap((r) => (r.kind === "file" ? [r.entry.path] : []));
  const left = status.labels.ours.refName ?? status.labels.ours.role;
  const right = status.labels.theirs.refName ?? status.labels.theirs.role;
  const selected = new Set(selection);
  const allShown = shown.length > 0 && shown.every((p) => selected.has(p));

  const renderRow = (row: Row) => {
    if (row.kind === "header") {
      return (
        <div className={styles.folder}>
          <span className={styles.folderName}>{row.dir}</span>
          <span className={styles.count}>{row.count}</span>
        </div>
      );
    }
    return (
      <FileRow
        entry={row.entry}
        selected={selected.has(row.entry.path)}
        active={row.entry.path === activePath}
        onToggle={() => onToggle(row.entry.path)}
        onOpen={() => onOpen(row.entry.path)}
        onAccept={(side) => onAccept([row.entry.path], side)}
      />
    );
  };

  return (
    <aside aria-label="Conflicted files" className={styles.panel}>
      <div className={styles.controls}>
        <div className={styles.filterRow}>
          <label className={styles.filter}>
            <input
              type="search"
              placeholder="Filter files"
              aria-label="Filter files"
              value={filter}
              onChange={(e) => onFilter(e.target.value)}
            />
          </label>
          <div role="group" aria-label="View" className={styles.seg}>
            {(["flat", "folders"] as const).map((v) => (
              <button
                key={v}
                type="button"
                aria-pressed={view === v}
                onClick={() => onView(v)}
              >
                {v === "flat" ? "Flat" : "Folders"}
              </button>
            ))}
          </div>
        </div>
        <div className={styles.selection}>
          <label className={styles.selectAll}>
            <input
              type="checkbox"
              aria-label="Select all shown files"
              checked={allShown}
              disabled={shown.length === 0}
              onChange={() =>
                allShown ? onClearSelection() : onSelectAll(shown)
              }
            />
          </label>
          <span className={styles.selected} aria-live="polite">
            {selection.length} selected
          </span>
          <button
            type="button"
            className={styles.btn}
            disabled={selection.length === 0}
            title={`Accept Left (${left}) for selected files`}
            onClick={() => onAccept(selection, "Ours")}
          >
            Accept Left
          </button>
          <button
            type="button"
            className={styles.btn}
            disabled={selection.length === 0}
            title={`Accept Right (${right}) for selected files`}
            onClick={() => onAccept(selection, "Theirs")}
          >
            Accept Right
          </button>
        </div>
      </div>

      {status.conflicts.length === 0 ? (
        <p className={styles.empty}>
          {status.operation.kind === "None"
            ? "No conflicts to resolve"
            : "All conflicts resolved"}
        </p>
      ) : rows.length === 0 ? (
        <p className={styles.empty}>No files match “{filter}”</p>
      ) : (
        <VirtualList
          className={styles.list}
          label="Conflicted files"
          items={rows}
          rowHeight={ROW_HEIGHT}
          getKey={(r) => r.key}
          render={renderRow}
        />
      )}

      <ResolvedSection items={resolved} onReopen={onReopen} />
    </aside>
  );
}

interface FileRowProps {
  entry: ConflictEntry;
  selected: boolean;
  active: boolean;
  onToggle: () => void;
  onOpen: () => void;
  onAccept: (side: AcceptSide) => void;
}

function FileRow({
  entry,
  selected,
  active,
  onToggle,
  onOpen,
  onAccept,
}: FileRowProps) {
  const { dir, file } = splitPath(entry.display);
  const changes = sideChanges(entry.conflictType);
  const badge = classBadge(entry);
  const deleted = (text: string) => (text === "Deleted" ? styles.deleted : "");
  return (
    <div
      className={`${styles.row} ${active ? styles.active : selected ? styles.picked : ""}`}
      data-testid="conflict-row"
    >
      <input
        type="checkbox"
        className={styles.check}
        aria-label={`Select ${file}`}
        checked={selected}
        onChange={onToggle}
      />
      <button
        type="button"
        className={styles.open}
        aria-label={`Merge ${entry.display}`}
        onClick={onOpen}
      >
        <span className={styles.line1}>
          <span className={styles.file}>{file}</span>
          <span className={styles.dir}>{dir}</span>
          {badge && <span className={styles.classTag}>{badge}</span>}
        </span>
        <span className={styles.line2}>
          <span>{typeLabel(entry.conflictType)}</span>
          <span
            aria-label={`Left: ${changes.left}, Right: ${changes.right}`}
            title={`Left: ${changes.left} · Right: ${changes.right}`}
          >
            <span className={deleted(changes.left)}>{changes.left}</span>
            {" / "}
            <span className={deleted(changes.right)}>{changes.right}</span>
          </span>
        </span>
      </button>
      <span className={styles.rowActions}>
        <button
          type="button"
          className={styles.mini}
          aria-label={`Accept Left for ${file}`}
          onClick={() => onAccept("Ours")}
        >
          L
        </button>
        <button
          type="button"
          className={styles.mini}
          aria-label={`Accept Right for ${file}`}
          onClick={() => onAccept("Theirs")}
        >
          R
        </button>
      </span>
    </div>
  );
}
