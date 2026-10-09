import { normalizeEol } from "../../model/text";

/** The lines of `text` without a trailing empty line. */
export function linesOf(text: string): string[] {
  const parts = normalizeEol(text).split("\n");
  if (parts[parts.length - 1] === "") parts.pop();
  return parts;
}
