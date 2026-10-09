import { describe, expect, it } from "vitest";
import { formatShortcut, matchShortcut, type KeyLike } from "./keymap";

const ev = (over: Partial<KeyLike>): KeyLike => ({
  key: "",
  code: "",
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...over,
});

describe("Keyboard shortcuts", () => {
  it("matches navigation shortcuts", () => {
    expect(matchShortcut(ev({ key: "ArrowDown", altKey: true }), true)).toBe(
      "nextChange",
    );
    expect(matchShortcut(ev({ key: "ArrowUp", altKey: true }), false)).toBe(
      "prevChange",
    );
    expect(matchShortcut(ev({ key: "F7" }), true)).toBe("nextConflict");
    expect(matchShortcut(ev({ key: "F7", shiftKey: true }), true)).toBe(
      "prevConflict",
    );
  });

  it("uses Cmd on macOS and Ctrl elsewhere", () => {
    expect(
      matchShortcut(
        ev({ key: "ArrowLeft", metaKey: true, altKey: true }),
        true,
      ),
    ).toBe("applyLeft");
    expect(
      matchShortcut(
        ev({ key: "ArrowLeft", ctrlKey: true, altKey: true }),
        true,
      ),
    ).toBeNull();
    expect(
      matchShortcut(
        ev({ key: "ArrowRight", ctrlKey: true, altKey: true }),
        false,
      ),
    ).toBe("applyRight");
    expect(
      matchShortcut(
        ev({ key: "Backspace", ctrlKey: true, altKey: true }),
        false,
      ),
    ).toBe("ignore");
    expect(matchShortcut(ev({ code: "KeyS", metaKey: true }), true)).toBe(
      "save",
    );
    expect(matchShortcut(ev({ code: "KeyS", ctrlKey: true }), false)).toBe(
      "save",
    );
    expect(matchShortcut(ev({ code: "KeyF", ctrlKey: true }), false)).toBe(
      "find",
    );
  });

  it("matches bulk shortcuts by physical key (Alt changes the produced character on macOS)", () => {
    expect(
      matchShortcut(
        ev({ key: "µ", code: "KeyM", metaKey: true, altKey: true }),
        true,
      ),
    ).toBe("resolveSimple");
    expect(
      matchShortcut(
        ev({ key: "å", code: "KeyA", metaKey: true, altKey: true }),
        true,
      ),
    ).toBe("applyNonConflicting");
  });

  it("ignores unrelated keys", () => {
    expect(matchShortcut(ev({ key: "a" }), true)).toBeNull();
    expect(matchShortcut(ev({ key: "ArrowDown" }), true)).toBeNull();
  });

  it("formats shortcuts per platform", () => {
    expect(formatShortcut(["Mod", "Alt", "Left"], true)).toBe("⌘⌥←");
    expect(formatShortcut(["Mod", "Alt", "Left"], false)).toBe("Ctrl+Alt+←");
  });
});
