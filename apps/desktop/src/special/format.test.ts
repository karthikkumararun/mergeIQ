import { describe, expect, it } from "vitest";
import {
  abbreviateOid,
  ago,
  baseName,
  formatBytes,
  shortDate,
  shortOid,
} from "./format";

describe("Special panel formatting", () => {
  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(18.4 * 1024)).toBe("18.4 KB");
    expect(formatBytes(38.2 * 1024 * 1024)).toBe("38.2 MB");
    expect(formatBytes(150 * 1024 * 1024)).toBe("150 MB");
    expect(formatBytes(null)).toBe("—");
  });

  it("abbreviates object ids", () => {
    expect(shortOid("a41f0c2e9d")).toBe("a41f0c2");
    expect(shortOid(null)).toBe("—");
    expect(
      abbreviateOid(
        "sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
      ),
    ).toBe("4d7a21…2393");
    expect(abbreviateOid("abc")).toBe("abc");
  });

  it("formats relative and short dates", () => {
    const now = Date.parse("2026-10-09T12:00:00Z");
    expect(ago("2026-10-07T12:00:00Z", now)).toBe("2d");
    expect(ago("2026-10-09T10:00:00Z", now)).toBe("2h");
    expect(ago("2026-10-09T11:50:00Z", now)).toBe("10m");
    expect(ago("2026-10-09T12:00:00Z", now)).toBe("just now");
    expect(ago(null, now)).toBe("");
    expect(ago("nonsense", now)).toBe("");
    expect(shortDate("2026-09-30T08:00:00Z")).toBe("Sep 30");
    expect(shortDate("2026-10-04T23:59:00Z")).toBe("Oct 4");
    expect(shortDate(undefined)).toBe("");
  });

  it("takes the last path segment", () => {
    expect(baseName("vendor/ui-kit")).toBe("ui-kit");
    expect(baseName("ui-kit")).toBe("ui-kit");
  });
});
