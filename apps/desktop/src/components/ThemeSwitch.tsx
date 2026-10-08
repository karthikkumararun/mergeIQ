import type { ThemeMode } from "../store/useAppStore";
import styles from "./ThemeSwitch.module.css";

const OPTIONS: { mode: ThemeMode; label: string }[] = [
  { mode: "light", label: "Light" },
  { mode: "dark", label: "Dark" },
  { mode: "system", label: "System" },
];

interface ThemeSwitchProps {
  value: ThemeMode;
  onChange: (mode: ThemeMode) => void;
}

export function ThemeSwitch({ value, onChange }: ThemeSwitchProps) {
  return (
    <div className={styles.group} role="group" aria-label="Theme">
      {OPTIONS.map(({ mode, label }) => (
        <button
          key={mode}
          type="button"
          className={styles.option}
          aria-pressed={value === mode}
          onClick={() => onChange(mode)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
