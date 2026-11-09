import { describe, expect, it } from "vitest";
import type { Commit } from "./api";
import { clickCommit, headIndex, isSelected, relativeDate, selectionDiff } from "./history";

const mk = (id: string): Commit => ({
  id,
  short: id,
  parents: [],
  base: `${id}^`,
  subject: "s",
  author: "a",
  date: "2026-01-01T00:00:00Z",
  refs: [],
});
const commits = [mk("c"), mk("b"), mk("a")]; // newest first

describe("selection", () => {
  it("single click selects one commit", () => {
    const sel = clickCommit(null, 1, false);
    expect(selectionDiff(commits, sel)).toEqual({ base: "b^", head: "b" });
  });
  it("shift-click builds a range regardless of click order", () => {
    const first = clickCommit(null, 2, false);
    const sel = clickCommit(first, 0, true);
    expect(selectionDiff(commits, sel)).toEqual({ base: "a^", head: "c" });
    expect([0, 1, 2].map((i) => isSelected(sel, i))).toEqual([true, true, true]);
    expect(commits[headIndex(sel) ?? -1]?.id).toBe(selectionDiff(commits, sel)?.head);
  });
  it("only the head of a selection is computed; the working tree has no commit row", () => {
    expect(headIndex(clickCommit(clickCommit(null, 2, false), 0, true))).toBe(0);
    expect(headIndex({ kind: "worktree" })).toBeNull();
    expect(headIndex(null)).toBeNull();
  });
  it("shift without a prior commit acts as a plain click", () => {
    const sel = clickCommit({ kind: "worktree" }, 1, true);
    expect(selectionDiff(commits, sel)).toEqual({ base: "b^", head: "b" });
    expect(isSelected(sel, 0)).toBe(false);
  });
  it("working tree is null/null", () => {
    expect(selectionDiff(commits, { kind: "worktree" })).toEqual({ base: null, head: null });
  });
  it("out-of-range selection yields null", () => {
    expect(selectionDiff([], clickCommit(null, 0, false))).toBeNull();
  });
});

describe("relativeDate", () => {
  const now = Date.parse("2026-11-06T12:00:00Z");
  it("formats units", () => {
    expect(relativeDate("2026-11-06T11:59:30Z", now)).toBe("just now");
    expect(relativeDate("2026-11-06T11:55:00Z", now)).toBe("5 min ago");
    expect(relativeDate("2026-11-06T09:00:00Z", now)).toBe("3 h ago");
    expect(relativeDate("2026-11-03T12:00:00Z", now)).toBe("3 d ago");
    expect(relativeDate("2026-10-06T12:00:00Z", now)).toBe("1 mo ago");
    expect(relativeDate("2024-11-06T12:00:00Z", now)).toBe("2 y ago");
  });
  it("returns input when invalid", () => {
    expect(relativeDate("nope", now)).toBe("nope");
  });
});
