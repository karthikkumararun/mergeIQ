import { describe, expect, it } from "vitest";
import { relativeTime } from "./relativeTime";

const now = new Date("2026-10-09T12:00:00Z");

describe("Commit context", () => {
  it("formats relative dates", () => {
    expect(relativeTime("2026-10-09T11:59:40Z", now)).toBe("now");
    expect(relativeTime("2026-10-09T11:30:00Z", now)).toBe("30m");
    expect(relativeTime("2026-10-09T10:00:00Z", now)).toBe("2h");
    expect(relativeTime("2026-10-07T12:00:00Z", now)).toBe("2d");
    expect(relativeTime("2026-09-18T12:00:00Z", now)).toBe("3w");
    expect(relativeTime("2026-06-01T12:00:00Z", now)).toBe("4mo");
    expect(relativeTime("2024-10-01T12:00:00Z", now)).toBe("2y");
    expect(relativeTime("not a date", now)).toBe("");
  });
});
