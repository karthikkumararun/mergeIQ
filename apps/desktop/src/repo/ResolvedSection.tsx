import type { ResolvedItem } from "./repoStore";
import styles from "./ConflictsPanel.module.css";

interface Props {
  items: ResolvedItem[];
  onReopen: (path: string) => void;
}

/** "Resolved in this session · N": what was resolved here and how, with Reopen conflict. */
export function ResolvedSection({ items, onReopen }: Props) {
  if (items.length === 0) return null;
  return (
    <details className={styles.resolved}>
      <summary>Resolved in this session · {items.length}</summary>
      <ul className={styles.resolvedList}>
        {items.map((item) => (
          <li key={item.path} className={styles.resolvedItem}>
            <span className={styles.resolvedPath} title={item.display}>
              {item.display}
            </span>
            <span className={styles.method}>{item.method}</span>
            <button
              type="button"
              className={styles.reopen}
              aria-label={`Reopen conflict ${item.display}`}
              onClick={() => onReopen(item.path)}
            >
              Reopen conflict…
            </button>
          </li>
        ))}
      </ul>
    </details>
  );
}
