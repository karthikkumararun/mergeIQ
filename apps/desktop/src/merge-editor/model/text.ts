import type { LineRange, SideText, Terminator } from "../../ipc/bindings";

/**
 * Splits decoded engine text into line contents (terminators removed).
 * Mirrors the engine's line table: `\n`, `\r\n` and lone `\r` terminate a line and a
 * trailing terminator does not start an extra empty line. Byte offsets in
 * `SideText.lines` are deliberately not used (they are UTF-8, JS strings are UTF-16).
 */
export function splitLines(text: string): string[] {
  if (text === "") return [];
  const parts = text.split(/\r\n|\n|\r/);
  if (parts[parts.length - 1] === "") parts.pop();
  return parts;
}

export function sideLines(side: SideText): string[] {
  return splitLines(side.text);
}

/** `lines[start..end]` as a block where every line ends with `\n`. */
export function block(lines: string[], range: LineRange): string {
  let out = "";
  for (let i = range.start; i < range.end; i++) out += `${lines[i]}\n`;
  return out;
}

/** The whole document text with `\n` terminators after every line. */
export function documentText(lines: string[]): string {
  return lines.map((l) => `${l}\n`).join("");
}

/** Normalizes any engine/user text to `\n` line endings. */
export function normalizeEol(text: string): string {
  return text.replace(/\r\n|\r/g, "\n");
}

export function eolString(term: Terminator): string {
  switch (term) {
    case "Crlf":
      return "\r\n";
    case "Cr":
      return "\r";
    case "Lf":
      return "\n";
    default:
      return "\n";
  }
}
