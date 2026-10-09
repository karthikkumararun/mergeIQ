import { useState } from "react";
import { ThemeSwitch } from "../components/ThemeSwitch";
import { CommandLine } from "../settings/CommandLine";
import { ipcCliApi, type CliSetupApi } from "../settings/cliApi";
import { useTheme } from "../theme/useTheme";
import styles from "./Settings.module.css";

export type SettingsSection = "general" | "cli";

interface SettingsProps {
  onClose: () => void;
  section?: SettingsSection;
  /** Backend calls for the Command line section (replaced in tests). */
  cliApi?: CliSetupApi;
  onCliChanged?: () => void;
}

export function Settings({
  onClose,
  section: initial = "general",
  cliApi = ipcCliApi,
  onCliChanged,
}: SettingsProps) {
  const { theme, setTheme } = useTheme();
  const [section, setSection] = useState<SettingsSection>(initial);
  const items: { id: SettingsSection; label: string }[] = [
    { id: "general", label: "General" },
    { id: "cli", label: "Command line" },
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
          </section>
        ) : (
          <CommandLine api={cliApi} onChanged={onCliChanged} />
        )}
      </div>
    </div>
  );
}
