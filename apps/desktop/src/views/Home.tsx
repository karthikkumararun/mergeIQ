import { useEffect, useState } from "react";
import { commands, type AppInfo } from "../ipc/bindings";
import { ThemeSwitch } from "../components/ThemeSwitch";
import { useTheme } from "../theme/useTheme";
import styles from "./Home.module.css";

const PLATFORM_LABEL: Record<string, string> = {
  macos: "macOS",
  windows: "Windows",
  linux: "Linux",
};

interface HomeProps {
  onOpenSettings: () => void;
}

export function Home({ onOpenSettings }: HomeProps) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const { theme, setTheme } = useTheme();

  useEffect(() => {
    commands.appInfo().then(setAppInfo);
  }, []);

  const versionLabel = appInfo
    ? `v${appInfo.version} · ${PLATFORM_LABEL[appInfo.platform] ?? appInfo.platform}`
    : null;

  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <div className={styles.brand}>
          <Logo />
          <span className={styles.name}>MergeIQ</span>
          {versionLabel && (
            <span className={styles.versionChip}>{versionLabel}</span>
          )}
        </div>
        <div className={styles.actions}>
          <ThemeSwitch value={theme} onChange={setTheme} />
          <button
            type="button"
            className={styles.iconButton}
            aria-label="Settings"
            onClick={onOpenSettings}
          >
            <SettingsIcon />
          </button>
        </div>
      </header>
      {/* Open/drop/recents and "Set up" cards are built by repo-browser, mergetool-cli
          and ai-assist — this change only provides the shell. */}
      <main className={styles.body} />
    </div>
  );
}

function Logo() {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="var(--accent)"
      strokeWidth="2.2"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M5 3v5c0 4 7 4 7 8v5" />
      <path d="M19 3v5c0 4-7 4-7 8" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
    </svg>
  );
}
