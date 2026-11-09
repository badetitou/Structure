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

/** A git ref for display: full SHAs shortened to 7 characters, names kept. */
export function shortSha(ref: string): string {
  return /^[0-9a-f]{40}$/i.test(ref) ? ref.slice(0, 7) : ref;
}

/** Short title for the range: "abc1234", "abc1234..def5678" or "working tree". */
export function rangeLabel(base: string | null, head: string | null): string {
  if (base === null) return "working tree";
  if (head === null) return `${shortSha(base)}..working tree`;
  return `${shortSha(base)}..${shortSha(head)}`;
}
