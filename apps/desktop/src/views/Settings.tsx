import { useEffect, useState } from "react";
import { ThemeSwitch } from "../components/ThemeSwitch";
import { ipcAiApi, type AiApi } from "../ai/api";
import { AiSettings } from "../settings/AiSettings";
import { CommandLine } from "../settings/CommandLine";
import { Lockfiles } from "../settings/Lockfiles";
import {
  ipcLockfileApi,
  type LockfileSettingsApi,
} from "../settings/lockfileApi";
import { ipcCliApi, type CliSetupApi } from "../settings/cliApi";
import { loadMergeSettings, saveMergeSettings } from "../merge-editor/hosts";
import type { MergeSettings } from "../merge-editor/model/types";
import { useTheme } from "../theme/useTheme";
import styles from "./Settings.module.css";

export type SettingsSection = "general" | "cli" | "lockfiles" | "ai";

interface SettingsProps {
  onClose: () => void;
  section?: SettingsSection;
  /** Backend calls for the Command line section (replaced in tests). */
  cliApi?: CliSetupApi;
  /** Backend calls for the Lockfiles section (replaced in tests). */
  lockfileApi?: LockfileSettingsApi;
  /** Backend calls for the AI section (replaced in tests). */
  aiApi?: AiApi;
  onCliChanged?: () => void;
}

export function Settings({
  onClose,
  section: initial = "general",
  cliApi = ipcCliApi,
  lockfileApi = ipcLockfileApi,
  aiApi = ipcAiApi,
  onCliChanged,
}: SettingsProps) {
  const { theme, setTheme } = useTheme();
  const [section, setSection] = useState<SettingsSection>(initial);
  const [merge, setMerge] = useState<MergeSettings | null>(null);
  useEffect(() => {
    void loadMergeSettings().then(setMerge);
  }, []);
  const setAutoAdvance = (autoAdvanceAfterSave: boolean) => {
    if (!merge) return;
    const next = { ...merge, autoAdvanceAfterSave };
    setMerge(next);
    void saveMergeSettings(next);
  };
  const items: { id: SettingsSection; label: string }[] = [
    { id: "general", label: "General" },
    { id: "cli", label: "Command line" },
    { id: "lockfiles", label: "Lockfiles" },
    { id: "ai", label: "AI" },
  ];

  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <h1 className={styles.title}>Settings</h1>
        <button type="button" className={styles.closeButton} onClick={onClose}>
          Done
        </button>
      </header>
      <div className={styles.layout}>
        <nav aria-label="Settings sections" className={styles.nav}>
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              className={styles.navItem}
              aria-current={section === item.id ? "page" : undefined}
              onClick={() => setSection(item.id)}
            >
              {item.label}
            </button>
          ))}
        </nav>
        {section === "general" ? (
          <section className={styles.content}>
            <h2 className={styles.sectionTitle}>General</h2>
            <div className={styles.row}>
              <span className={styles.label}>Theme</span>
              <ThemeSwitch value={theme} onChange={setTheme} />
            </div>
            <label className={styles.row}>
              <span className={styles.label}>
                After saving a resolved file, open the next conflicted file
              </span>
              <input
                type="checkbox"
                checked={merge?.autoAdvanceAfterSave ?? true}
                disabled={!merge}
                onChange={(e) => setAutoAdvance(e.target.checked)}
              />
            </label>
          </section>
        ) : section === "cli" ? (
          <CommandLine api={cliApi} onChanged={onCliChanged} />
        ) : section === "lockfiles" ? (
          <Lockfiles api={lockfileApi} />
        ) : (
          <AiSettings api={aiApi} />
        )}
      </div>
    </div>
  );
}
