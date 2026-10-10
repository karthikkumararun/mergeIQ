import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { commands } from "../ipc/bindings";
import { isIpcMock } from "../ipc/mock";
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
 * Title bar, frame and window background are native: follow the chosen mode (null = follow the
 * OS) and paint the window with the theme's `--bg` so no white rim shows around dark content.
 */
function applyNativeChrome(mode: ThemeMode) {
  try {
    const win = getCurrentWindow();
    const report = (error: unknown) =>
      console.warn("native theme not applied", error);
    win.setTheme(mode === "system" ? null : mode).catch(report);
    const bg = getComputedStyle(document.documentElement)
      .getPropertyValue("--bg")
      .trim();
    if (bg) win.setBackgroundColor(bg).catch(report);
  } catch {
    // No Tauri runtime (plain browser, unit tests): nothing native to theme.
  }
}

/**
 * Applies the current theme to `<html data-theme>`, follows OS `prefers-color-scheme`
 * live while in `system` mode, and persists manual choices via the settings backend.
 */
export function useTheme() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);

  useEffect(() => {
    // Mocked runs (dev routes, Playwright) pick the theme from the URL and have no backend.
    if (isIpcMock) return;
    commands.getSettings().then((settings) => {
      setTheme(settings.theme as ThemeMode);
    });
  }, [setTheme]);

  useEffect(() => {
    if (isIpcMock) return;
    const apply = () => {
      document.documentElement.setAttribute("data-theme", resolveTheme(theme));
      applyNativeChrome(theme);
    };
    apply();

    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  const updateTheme = (mode: ThemeMode) => {
    setTheme(mode);
    if (isIpcMock) return;
    void commands.updateSettings({ theme: mode }).then((result) => {
      if (result.status === "error") {
        console.error("Failed to persist theme setting", result.error);
      }
    });
  };

  return { theme, setTheme: updateTheme };
}
