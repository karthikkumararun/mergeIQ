import {
  commands,
  type Analysis,
  type StructuralResolve,
} from "../../../ipc/bindings";
import { isIpcMock, mockStructuralResolve } from "../../../ipc/mock";
import { describeError } from "../../hosts";

/** Asks the backend for structural proposals (the mocked backend in Playwright runs). */
export async function resolveStructural(
  path: string,
  analysis: Analysis,
): Promise<StructuralResolve> {
  if (isIpcMock) return mockStructuralResolve(path, analysis);
  const result = await commands.structuralResolve(path, analysis);
  if (result.status === "error") throw new Error(describeError(result.error));
  return result.data;
}

export type ResolveStructural = typeof resolveStructural;
