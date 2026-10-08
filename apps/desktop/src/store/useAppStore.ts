import { create } from "zustand";

export type ThemeMode = "system" | "light" | "dark";

interface AppState {
  theme: ThemeMode;
  setTheme: (theme: ThemeMode) => void;
}

export const useAppStore = create<AppState>((set) => ({
  theme: "system",
  setTheme: (theme) => set({ theme }),
}));
