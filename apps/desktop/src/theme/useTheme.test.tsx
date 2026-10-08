import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useTheme } from "./useTheme";

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
});
