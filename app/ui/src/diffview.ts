import type { Diff, FileDiff, FileStatus } from "./api";

/** Files with more shown lines than this start collapsed in the diff panel. */
export const OPEN_MAX_LINES = 400;

/** One-letter status, as in `git status --short`. */
export function statusLetter(s: FileStatus): string {
  return { added: "A", deleted: "D", modified: "M", renamed: "R" }[s];
}

/** "old → new" for a rename, else the path. */
export function fileLabel(f: FileDiff): string {
  return f.old_path && f.old_path !== f.path ? `${f.old_path} → ${f.path}` : f.path;
}

export function shownLines(f: FileDiff): number {
  return f.hunks.reduce((n, h) => n + h.lines.length, 0);
}

/** Whether a file starts expanded: small enough and something to show. */
export function startsOpen(f: FileDiff): boolean {
  const n = shownLines(f);
  return n > 0 && n <= OPEN_MAX_LINES;
}

export interface DiffTotals {
  files: number;
  additions: number;
  deletions: number;
}

export function totals(d: Diff): DiffTotals {
  return d.files.reduce(
    (t, f) => ({ files: t.files + 1, additions: t.additions + f.additions, deletions: t.deletions + f.deletions }),
    { files: 0, additions: 0, deletions: 0 },
  );
}

/** Short title for the range: "abc1234", "abc1234..def5678" or "working tree". */
export function rangeLabel(base: string | null, head: string | null): string {
  const short = (r: string) => (/^[0-9a-f]{40}$/.test(r) ? r.slice(0, 7) : r);
  if (base === null) return "working tree";
  if (head === null) return `${short(base)}..working tree`;
  return `${short(base)}..${short(head)}`;
}
