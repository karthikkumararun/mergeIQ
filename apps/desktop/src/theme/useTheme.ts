import { useEffect } from "react";
import { commands } from "../ipc/bindings";
import { useAppStore, type ThemeMode } from "../store/useAppStore";

function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return mode;
}

/**
 * Applies the current theme to `<html data-theme>`, follows OS `prefers-color-scheme`
 * live while in `system` mode, and persists manual choices via the settings backend.
 */
export function useTheme() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);

  useEffect(() => {
    commands.getSettings().then((settings) => {
      setTheme(settings.theme as ThemeMode);
    });
  }, [setTheme]);

  useEffect(() => {
    const apply = () => {
      document.documentElement.setAttribute("data-theme", resolveTheme(theme));
    };
    apply();

    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  const updateTheme = (mode: ThemeMode) => {
    setTheme(mode);
    void commands.updateSettings({ theme: mode }).then((result) => {
      if (result.status === "error") {
        console.error("Failed to persist theme setting", result.error);
      }
    });
  };

  return { theme, setTheme: updateTheme };
}
