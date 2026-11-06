import type { Commit } from "./api";

/** Selected diff: the working tree, or commits `newer..older` (list indices, lower = newer). */
export type Selection =
  | { kind: "worktree" }
  | { kind: "range"; newer: number; older: number; anchor: number };

export interface Diff {
  base: string | null;
  head: string | null;
}

/** Next selection after a click. Shift on a commit extends from the previous commit anchor. */
export function clickCommit(prev: Selection | null, index: number, shift: boolean): Selection {
  if (shift && prev?.kind === "range") {
    return {
      kind: "range",
      newer: Math.min(prev.anchor, index),
      older: Math.max(prev.anchor, index),
      anchor: prev.anchor,
    };
  }
  return { kind: "range", newer: index, older: index, anchor: index };
}

export function isSelected(sel: Selection | null, index: number): boolean {
  return sel?.kind === "range" && index >= sel.newer && index <= sel.older;
}

/** base/head to pass to `impact`; null when the selection does not match the commits. */
export function selectionDiff(commits: Commit[], sel: Selection): Diff | null {
  if (sel.kind === "worktree") return { base: null, head: null };
  const newer = commits[sel.newer];
  const older = commits[sel.older];
  if (!newer || !older) return null;
  return { base: older.base, head: newer.id };
}

const UNITS: [number, string][] = [
  [365 * 86400, "y"],
  [30 * 86400, "mo"],
  [7 * 86400, "w"],
  [86400, "d"],
  [3600, "h"],
  [60, "min"],
];

/** "just now", "5 min ago", "3 d ago"; falls back to the input if unparsable. */
export function relativeDate(iso: string, now: number = Date.now()): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return iso;
  const secs = Math.floor((now - t) / 1000);
  for (const [size, label] of UNITS) {
    if (secs >= size) return `${Math.floor(secs / size)} ${label} ago`;
  }
  return "just now";
}
