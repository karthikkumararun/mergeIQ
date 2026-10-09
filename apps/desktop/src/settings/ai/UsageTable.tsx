import { useState } from "react";
import type { Price, SessionUsage } from "../../ipc/bindings";
import { cost, sessionLine } from "../../ai/format";
import styles from "./AiSettings.module.css";

interface Props {
  usage: SessionUsage | null;
  prices: Record<string, Price>;
  onPrices: (prices: Record<string, Price>) => void;
  onReset: () => void;
}

const FIELDS: [keyof Price, string][] = [
  ["input", "Input"],
  ["output", "Output"],
  ["cacheRead", "Cache read"],
  ["cacheWrite", "Cache write"],
];

function Num({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: number;
  onCommit: (n: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setDraft(String(value));
  }
  const commit = () => {
    const n = Number(draft);
    if (Number.isFinite(n) && n >= 0 && n !== value) onCommit(n);
    else setDraft(String(value));
  };
  return (
    <input
      type="number"
      min={0}
      step="any"
      aria-label={label}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}

/** Session usage and the editable price table (US dollars per million tokens). */
export function UsageTable({ usage, prices, onPrices, onReset }: Props) {
  const [model, setModel] = useState("");
  const rows = Object.entries(prices).sort(([a], [b]) => a.localeCompare(b));

  const add = () => {
    const id = model.trim();
    if (!id || id in prices) return;
    onPrices({
      ...prices,
      [id]: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    });
    setModel("");
  };

  return (
    <section className={styles.section} aria-labelledby="ai-usage-title">
      <h2 id="ai-usage-title" className={styles.h2}>
        Usage and prices
      </h2>
      <div className={styles.row}>
        <span className={styles.usageLine} data-testid="ai-usage-summary">
          {usage && usage.requests > 0
            ? sessionLine(usage.requests, usage.totals, usage.cost)
            : "No requests since MergeIQ started"}
        </span>
        <button
          type="button"
          className={`${styles.btn} ${styles.small}`}
          disabled={!usage || usage.requests === 0}
          onClick={onReset}
        >
          Reset
        </button>
      </div>
      <p className={styles.hint}>
        Estimated cost uses these prices, in US dollars per million tokens. A
        model matches the longest entry its id starts with. Edit them to match
        your rates; the defaults are only a starting point.
      </p>
      <table className={styles.table}>
        <thead>
          <tr>
            <th scope="col">Model</th>
            {FIELDS.map(([, label]) => (
              <th key={label} scope="col">
                {label}
              </th>
            ))}
            <th scope="col">
              <span className={styles.srOnly}>Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map(([id, price]) => (
            <tr key={id}>
              <td className={styles.modelCell}>{id}</td>
              {FIELDS.map(([key, label]) => (
                <td key={key}>
                  <Num
                    label={`${id} ${label.toLowerCase()} price`}
                    value={price[key] ?? 0}
                    onCommit={(n) =>
                      onPrices({ ...prices, [id]: { ...price, [key]: n } })
                    }
                  />
                </td>
              ))}
              <td>
                <button
                  type="button"
                  className={`${styles.btn} ${styles.small}`}
                  aria-label={`Remove price for ${id}`}
                  onClick={() => {
                    const next = { ...prices };
                    delete next[id];
                    onPrices(next);
                  }}
                >
                  Remove
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className={styles.row}>
        <input
          className={styles.in}
          style={{ maxWidth: 340 }}
          type="text"
          placeholder="Model id or prefix, e.g. claude-sonnet-5"
          aria-label="Model to add a price for"
          value={model}
          spellCheck={false}
          onChange={(e) => setModel(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") add();
          }}
        />
        <button
          type="button"
          className={`${styles.btn} ${styles.small}`}
          disabled={!model.trim() || model.trim() in prices}
          onClick={add}
        >
          Add model
        </button>
        <span className={styles.hint}>
          {rows.length === 0 ? "No prices: costs are not estimated." : ""}
          {usage?.cost != null ? ` Session total ${cost(usage.cost)}.` : ""}
        </span>
      </div>
    </section>
  );
}
