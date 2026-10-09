import { useCallback, useEffect, useRef, useState } from "react";
import {
  failureMessage,
  failureOf,
  type AiApi,
  type AiSettings as AiSettingsDto,
  type KeyInfo,
  type ProviderKind,
  type RepoDecision,
  type SessionUsage,
} from "../ai/api";
import { full, providerOf, type FullAiSettings } from "../ai/defaults";
import styles from "./ai/AiSettings.module.css";
import { PrivacyForm } from "./ai/PrivacyForm";
import { ProviderForm } from "./ai/ProviderForm";
import { UsageTable } from "./ai/UsageTable";

interface Props {
  api: AiApi;
}

function toDto(s: FullAiSettings): AiSettingsDto {
  return {
    provider: s.provider,
    confirmed: s.confirmed,
    noticeAccepted: s.noticeAccepted,
    providers: s.providers,
    privacy: s.privacy,
    context: s.context,
    prices: s.prices,
  };
}

function ContextForm({
  surrounding,
  budget,
  onChange,
}: {
  surrounding: number;
  budget: number;
  onChange: (surrounding: number, budget: number) => void;
}) {
  const [s, setS] = useState(String(surrounding));
  const [b, setB] = useState(String(budget));
  const commit = () => {
    const sn = Math.min(500, Math.max(0, Math.round(Number(s))));
    const bn = Math.min(1_000_000, Math.max(2_000, Math.round(Number(b))));
    const nextS = Number.isFinite(sn) ? sn : surrounding;
    const nextB = Number.isFinite(bn) ? bn : budget;
    setS(String(nextS));
    setB(String(nextB));
    if (nextS !== surrounding || nextB !== budget) onChange(nextS, nextB);
  };
  return (
    <section className={styles.section} aria-labelledby="ai-context-title">
      <h2 id="ai-context-title" className={styles.h2}>
        Context
      </h2>
      <div className={styles.grid2}>
        <label className={styles.fld}>
          Surrounding lines
          <input
            className={styles.in}
            type="number"
            min={0}
            max={500}
            value={s}
            onChange={(e) => setS(e.target.value)}
            onBlur={commit}
          />
        </label>
        <label className={styles.fld}>
          Input token budget
          <input
            className={styles.in}
            type="number"
            min={2000}
            step={1000}
            value={b}
            onChange={(e) => setB(e.target.value)}
            onBlur={commit}
          />
        </label>
      </div>
      <p className={styles.hint}>
        Over budget, full files are dropped first, then commit bodies, then
        surrounding lines. The conflict itself is always sent.
      </p>
    </section>
  );
}

/** Settings › AI: provider, privacy, context and usage (`ui/AiSettings.dc.html`). */
export function AiSettings({ api }: Props) {
  const [settings, setSettings] = useState<FullAiSettings | null>(null);
  const [key, setKey] = useState<KeyInfo | null>(null);
  const [usage, setUsage] = useState<SessionUsage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef<FullAiSettings | null>(null);

  const loadKey = useCallback(
    async (provider: ProviderKind) => {
      try {
        setKey(await api.keyInfo(provider));
      } catch {
        setKey(null);
      }
    },
    [api],
  );

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const loaded = full(await api.getSettings());
        if (!live) return;
        latest.current = loaded;
        setSettings(loaded);
        void loadKey(loaded.provider);
        setUsage(await api.usage());
      } catch (e) {
        if (live) setError(failureMessage(failureOf(e)));
      }
    })();
    return () => {
      live = false;
    };
  }, [api, loadKey]);

  /** Applies `edit` to the latest settings, shows it at once and saves it. */
  const save = useCallback(
    async (edit: (s: FullAiSettings) => FullAiSettings) => {
      const current = latest.current;
      if (!current) return;
      const next = edit(current);
      latest.current = next;
      setSettings(next);
      setError(null);
      try {
        const stored = full(await api.updateSettings(toDto(next)));
        latest.current = stored;
        setSettings(stored);
      } catch (e) {
        setError(failureMessage(failureOf(e)));
      }
    },
    [api],
  );

  if (!settings)
    return (
      <div className={styles.page}>
        <h1 className={styles.title}>AI</h1>
        {error ? (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        ) : (
          <p className={styles.text}>Loading…</p>
        )}
      </div>
    );

  const provider = settings.provider;
  const ps = providerOf(settings);

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>AI</h1>
      {error ? (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      ) : null}

      <ProviderForm
        api={api}
        provider={provider}
        settings={ps}
        keyInfo={key}
        onProvider={(kind) => {
          void save((s) => ({ ...s, provider: kind, confirmed: true }));
          setKey(null);
          void loadKey(kind);
        }}
        onSettings={(next) =>
          void save((s) => ({
            ...s,
            confirmed: true,
            providers: { ...s.providers, [s.provider]: next },
          }))
        }
        onKeyChanged={(info) => {
          setKey(info);
          void save((s) => ({ ...s, confirmed: true }));
        }}
      />

      <PrivacyForm
        globs={settings.privacy.excludeGlobs}
        repos={settings.privacy.repos}
        noticeAccepted={settings.noticeAccepted}
        onGlobs={(excludeGlobs) =>
          void save((s) => ({ ...s, privacy: { ...s.privacy, excludeGlobs } }))
        }
        onDecision={(scope, decision: RepoDecision | null) => {
          void (async () => {
            try {
              const stored = full(await api.setRepoDecision(scope, decision));
              latest.current = stored;
              setSettings(stored);
            } catch (e) {
              setError(failureMessage(failureOf(e)));
            }
          })();
        }}
        onNotice={(noticeAccepted) =>
          void save((s) => ({ ...s, noticeAccepted }))
        }
      />

      <ContextForm
        surrounding={settings.context.surroundingLines}
        budget={settings.context.tokenBudget}
        onChange={(surroundingLines, tokenBudget) =>
          void save((s) => ({
            ...s,
            context: { surroundingLines, tokenBudget },
          }))
        }
      />

      <UsageTable
        usage={usage}
        prices={settings.prices.entries}
        onPrices={(entries) =>
          void save((s) => ({ ...s, prices: { entries } }))
        }
        onReset={() => {
          void (async () => {
            await api.resetUsage();
            setUsage(await api.usage());
          })();
        }}
      />
    </div>
  );
}
