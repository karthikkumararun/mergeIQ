import { ThemeSwitch } from "../components/ThemeSwitch";
import { useTheme } from "../theme/useTheme";
import styles from "./Settings.module.css";

interface SettingsProps {
  onClose: () => void;
}

export function Settings({ onClose }: SettingsProps) {
  const { theme, setTheme } = useTheme();

  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <h1 className={styles.title}>Settings</h1>
        <button type="button" className={styles.closeButton} onClick={onClose}>
          Done
        </button>
      </header>
      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>General</h2>
        <div className={styles.row}>
          <span className={styles.label}>Theme</span>
          <ThemeSwitch value={theme} onChange={setTheme} />
        </div>
      </section>
    </div>
  );
}
