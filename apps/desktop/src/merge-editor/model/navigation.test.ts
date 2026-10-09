import { describe, expect, it } from "vitest";
import { fixture } from "../__fixtures__";
import { pickChunk } from "./navigation";
import { chunksOf } from "./session";
import { createResultState } from "./state";

describe("Navigation", () => {
  const a = fixture("mixed-changes");
  const chunks = chunksOf(createResultState(a));

  it("Next conflict", () => {
    const t = pickChunk(chunks, 0, 1, "conflict");
    expect(t?.chunk.id).toBe(1);
    expect(t?.wrapped).toBe(false);
  });

  it("Next change walks every chunk in order", () => {
    expect(pickChunk(chunks, 0, 1, "change")?.chunk.id).toBe(0);
    expect(pickChunk(chunks, chunks[0].from, 1, "change")?.chunk.id).toBe(1);
    expect(pickChunk(chunks, chunks[1].from, 1, "change")?.chunk.id).toBe(2);
  });

  it("Navigation wraps with a hint", () => {
    const t = pickChunk(chunks, chunks[2].from, 1, "change");
    expect(t).toMatchObject({ wrapped: true });
    expect(t?.chunk.id).toBe(0);
    const p = pickChunk(chunks, 0, -1, "change");
    expect(p).toMatchObject({ wrapped: true });
    expect(p?.chunk.id).toBe(2);
  });

  it("Previous conflict", () => {
    expect(pickChunk(chunks, chunks[2].from, -1, "conflict")?.chunk.id).toBe(1);
  });

  it("Returns null without candidates", () => {
    expect(pickChunk([], 0, 1, "change")).toBeNull();
    const none = chunks.filter((c) => c.kind !== "Conflict");
    expect(pickChunk(none, 0, 1, "conflict")).toBeNull();
  });
});
