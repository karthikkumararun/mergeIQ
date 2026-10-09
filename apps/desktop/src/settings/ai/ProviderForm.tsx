import { useState } from "react";
import {
  failureOf,
  failureMessage,
  type AiApi,
  type KeyInfo,
  type ProviderKind,
  type ProviderSettings,
} from "../../ai/api";
import {
  EFFORTS,
  hasEffort,
  PROVIDERS,
  SUGGESTED_MODELS,
} from "../../ai/defaults";
import { latency, PROVIDER_NAME } from "../../ai/format";
import type { Effort } from "../../ipc/bindings";
import styles from "./AiSettings.module.css";

interface Props {
  api: AiApi;
  provider: ProviderKind;
  settings: ProviderSettings;
  keyInfo: KeyInfo | null;
  onProvider: (kind: ProviderKind) => void;
  onSettings: (next: ProviderSettings) => void;
  onKeyChanged: (info: KeyInfo) => void;
}

type TestResult =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "ok"; text: string }
  | { kind: "fail"; text: string };

/** Text fields commit on blur or Enter so every keystroke is not a save. */
function Field({
  label,
  value,
  onCommit,
  list,
  mono = true,
}: {
  label: string;
  value: string;
  onCommit: (v: string) => void;
  list?: string;
  mono?: boolean;
}) {
  const [draft, setDraft] = useState(value);
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    // The stored value changed underneath (provider switch, reset).
    setShown(value);
    setDraft(value);
  }
  const commit = () => {
    const v = draft.trim();
    if (v && v !== value) onCommit(v);
    else setDraft(value);
  };
  return (
    <label className={styles.fld}>
      {label}
      <input
        className={styles.in}
        type="text"
        value={draft}
        list={list}
        spellCheck={false}
        style={mono ? undefined : { fontFamily: "var(--font-ui)" }}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
        }}
      />
    </label>
  );
}

/** Provider, model, URL, effort, key and "Test connection" (`ui/AiSettings.dc.html`). */
export function ProviderForm({
  api,
  provider,
  settings,
  keyInfo,
  onProvider,
  onSettings,
  onKeyChanged,
}: Props) {
  const [replacing, setReplacing] = useState(false);
  const [draftKey, setDraftKey] = useState("");
  const [keyError, setKeyError] = useState<string | null>(null);
  const [test, setTest] = useState<TestResult>({ kind: "idle" });
  const needsKey = provider !== "ollama";

  const saveKey = async () => {
    setKeyError(null);
    try {
      onKeyChanged(await api.setKey(provider, draftKey));
      setDraftKey("");
      setReplacing(false);
      setTest({ kind: "idle" });
    } catch (e) {
      setKeyError(failureMessage(failureOf(e)));
    }
  };

  const removeKey = async () => {
    try {
      onKeyChanged(await api.deleteKey(provider));
      setTest({ kind: "idle" });
    } catch (e) {
      setKeyError(failureMessage(failureOf(e)));
    }
  };

  const runTest = async () => {
    setTest({ kind: "running" });
    try {
      const report = await api.testConnection(
        provider,
        settings,
        draftKey.trim() ? draftKey : null,
      );
      setTest({
        kind: "ok",
        text: `Connected · ${report.model} responded in ${latency(report.latencyMs)}`,
      });
    } catch (e) {
      setTest({ kind: "fail", text: failureMessage(failureOf(e)) });
    }
  };

  return (
    <section className={styles.section} aria-labelledby="ai-provider-title">
      <h2 id="ai-provider-title" className={styles.h2}>
        Provider
      </h2>
      <div role="radiogroup" aria-label="Provider" className={styles.segGroup}>
        {PROVIDERS.map((p) => (
          <button
            key={p}
            type="button"
            role="radio"
            aria-checked={p === provider}
            className={styles.seg}
            onClick={() => {
              setTest({ kind: "idle" });
              setReplacing(false);
              setDraftKey("");
              onProvider(p);
            }}
          >
            {PROVIDER_NAME[p]}
          </button>
        ))}
      </div>

      <div className={styles.grid} key={provider}>
        <Field
          label="Model"
          value={settings.model}
          list="ai-models"
          onCommit={(model) => onSettings({ ...settings, model })}
        />
        <datalist id="ai-models">
          {SUGGESTED_MODELS[provider].map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
        <Field
          label="Base URL"
          value={settings.baseUrl}
          onCommit={(baseUrl) => onSettings({ ...settings, baseUrl })}
        />
        {hasEffort(provider) ? (
          <label className={styles.fld}>
            Effort
            <select
              className={`${styles.in}`}
              value={settings.effort}
              onChange={(e) =>
                onSettings({ ...settings, effort: e.target.value as Effort })
              }
            >
              {EFFORTS.map((e) => (
                <option key={e} value={e}>
                  {e}
                </option>
              ))}
            </select>
          </label>
        ) : null}
      </div>

      {needsKey ? (
        <>
          <div className={styles.keyRow}>
            <div className={styles.keyField}>
              <span id="ai-key-label">
                {provider === "github-models" ? "Token" : "API key"}
              </span>
              {replacing ? (
                <input
                  className={styles.in}
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  aria-labelledby="ai-key-label"
                  value={draftKey}
                  placeholder="Paste the key"
                  onChange={(e) => setDraftKey(e.target.value)}
                />
              ) : (
                <div
                  className={styles.keyBox}
                  role="group"
                  aria-labelledby="ai-key-label"
                >
                  <span className={styles.mask} data-testid="ai-key-mask">
                    {keyInfo?.present
                      ? `•••• •••• ${keyInfo.last4 ?? ""}`
                      : "No key stored"}
                  </span>
                  {keyInfo?.present ? (
                    <span className={styles.where}>in {keyInfo.storeName}</span>
                  ) : null}
                </div>
              )}
            </div>
            {replacing ? (
              <>
                <button
                  type="button"
                  className={`${styles.btn} ${styles.primary}`}
                  disabled={!draftKey.trim()}
                  onClick={() => void saveKey()}
                >
                  Save key
                </button>
                <button
                  type="button"
                  className={styles.btn}
                  onClick={() => {
                    setReplacing(false);
                    setDraftKey("");
                    setKeyError(null);
                  }}
                >
                  Cancel
                </button>
              </>
            ) : (
              <button
                type="button"
                className={styles.btn}
                onClick={() => setReplacing(true)}
              >
                {keyInfo?.present ? "Replace key…" : "Add key…"}
              </button>
            )}
            <button
              type="button"
              className={styles.btn}
              disabled={test.kind === "running"}
              onClick={() => void runTest()}
            >
              Test connection
            </button>
          </div>
          {keyInfo?.present && !replacing ? (
            <div className={styles.row}>
              <button
                type="button"
                className={styles.link}
                onClick={() => void removeKey()}
              >
                Remove key from {keyInfo.storeName}
              </button>
            </div>
          ) : null}
          {keyError ? (
            <p role="alert" className={styles.error}>
              {keyError}
            </p>
          ) : null}
        </>
      ) : (
        <div className={styles.row}>
          <p className={styles.text}>
            Requests go to this computer only. No API key is needed.
          </p>
          <button
            type="button"
            className={styles.btn}
            disabled={test.kind === "running"}
            onClick={() => void runTest()}
          >
            Test connection
          </button>
        </div>
      )}

      <div role="status" className={styles.result} data-testid="ai-test-result">
        {test.kind === "running" ? (
          <span>Testing…</span>
        ) : test.kind === "ok" ? (
          <span className={styles.ok}>
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
              style={{ verticalAlign: "-3px", marginRight: 8 }}
            >
              <path d="m5 12 5 5 9-10" />
            </svg>
            {test.text}
          </span>
        ) : test.kind === "fail" ? (
          <span className={styles.bad}>{test.text}</span>
        ) : null}
      </div>
    </section>
  );
}
