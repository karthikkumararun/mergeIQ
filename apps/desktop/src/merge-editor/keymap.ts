export type ShortcutId =
  | "nextChange"
  | "prevChange"
  | "nextConflict"
  | "prevConflict"
  | "applyLeft"
  | "applyRight"
  | "ignore"
  | "resolveSimple"
  | "applyNonConflicting"
  | "save"
  | "find";

export interface KeyLike {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export function isMacPlatform(): boolean {
  return (
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent)
  );
}

/** Maps a keyboard event to a merge-editor shortcut (Mod = Cmd on macOS, Ctrl elsewhere). */
export function matchShortcut(
  e: KeyLike,
  mac = isMacPlatform(),
): ShortcutId | null {
  const mod = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  const noMod = !e.metaKey && !e.ctrlKey;
  if (mod && e.altKey && !e.shiftKey) {
    switch (e.key) {
      case "ArrowLeft":
        return "applyLeft";
      case "ArrowRight":
        return "applyRight";
      case "Backspace":
        return "ignore";
    }
    if (e.code === "KeyM") return "resolveSimple";
    if (e.code === "KeyA") return "applyNonConflicting";
    return null;
  }
  if (mod && !e.altKey && !e.shiftKey && e.code === "KeyS") return "save";
  if (mod && !e.altKey && !e.shiftKey && e.code === "KeyF") return "find";
  if (noMod && e.altKey && !e.shiftKey) {
    if (e.key === "ArrowDown") return "nextChange";
    if (e.key === "ArrowUp") return "prevChange";
  }
  if (noMod && !e.altKey && e.key === "F7")
    return e.shiftKey ? "prevConflict" : "nextConflict";
  return null;
}

/** Renders a shortcut for display, e.g. `⌘⌥←` on macOS, `Ctrl+Alt+←` elsewhere. */
export function formatShortcut(parts: string[], mac = isMacPlatform()): string {
  const map: Record<string, string> = mac
    ? {
        Mod: "⌘",
        Alt: "⌥",
        Shift: "⇧",
        Left: "←",
        Right: "→",
        Up: "↑",
        Down: "↓",
        Backspace: "⌫",
      }
    : { Mod: "Ctrl", Left: "←", Right: "→", Up: "↑", Down: "↓" };
  const out = parts.map((p) => map[p] ?? p);
  return mac ? out.join("") : out.join("+");
}
