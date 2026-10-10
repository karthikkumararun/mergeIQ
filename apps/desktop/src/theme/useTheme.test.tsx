import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useTheme } from "./useTheme";

const setNativeTheme = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
const setNativeBackground = vi.hoisted(() =>
  vi.fn().mockResolvedValue(undefined),
);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setTheme: setNativeTheme,
    setBackgroundColor: setNativeBackground,
  }),
}));

vi.mock("../ipc/bindings", () => ({
  commands: {
    getSettings: vi.fn().mockResolvedValue({ theme: "system" }),
    updateSettings: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: { theme: "system" } }),
  },
}));

function mockMatchMedia(initialMatches: boolean) {
  const listeners: Array<() => void> = [];
  const mql = {
    matches: initialMatches,
    addEventListener: (_event: string, cb: () => void) => listeners.push(cb),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));
  return {
    trigger(matches: boolean) {
      mql.matches = matches;
      listeners.forEach((cb) => cb());
    },
  };
}

describe("useTheme", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("follows OS theme changes live while in system mode", async () => {
    const media = mockMatchMedia(false);
    const { result } = renderHook(() => useTheme());

    await waitFor(() => expect(result.current.theme).toBe("system"));
    await waitFor(() =>
      expect(document.documentElement.getAttribute("data-theme")).toBe("light"),
    );

    act(() => media.trigger(true));

    await waitFor(() =>
      expect(document.documentElement.getAttribute("data-theme")).toBe("dark"),
    );
  });

  it("sets the native window theme to the chosen mode and clears it for system", async () => {
    mockMatchMedia(false);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(setNativeTheme).toHaveBeenCalledWith(null));

    act(() => result.current.setTheme("dark"));
    await waitFor(() =>
      expect(setNativeTheme).toHaveBeenLastCalledWith("dark"),
    );

    document.documentElement.style.setProperty("--bg", "#16181c");
    act(() => result.current.setTheme("system"));
    await waitFor(() =>
      expect(setNativeBackground).toHaveBeenCalledWith("#16181c"),
    );

    act(() => result.current.setTheme("light"));
    await waitFor(() =>
      expect(setNativeTheme).toHaveBeenLastCalledWith("light"),
    );
  });
});
