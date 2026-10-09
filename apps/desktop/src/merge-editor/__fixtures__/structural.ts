import type { Analysis, StructuralResolve } from "../../ipc/bindings";

/** A file with its engine analysis and the proposals the engine computed for it. */
export interface StructuralFixture {
  path: string;
  analysis: Analysis;
  resolve: StructuralResolve;
}

const modules = import.meta.glob<StructuralFixture>("./structural/*.json", {
  eager: true,
  import: "default",
});

/** Engine-produced structural fixtures keyed by file name without extension. */
export const structuralFixtures: Record<string, StructuralFixture> =
  Object.fromEntries(
    Object.entries(modules).map(([path, f]) => [
      path.replace(/^\.\/structural\/(.*)\.json$/, "$1"),
      f,
    ]),
  );
