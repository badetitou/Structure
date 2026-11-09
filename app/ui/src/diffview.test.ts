import { describe, expect, it } from "vitest";
import type { FileDiff } from "./api";
import { OPEN_MAX_LINES, fileLabel, rangeLabel, shownLines, startsOpen, statusLetter, totals } from "./diffview";

const file = (over: Partial<FileDiff> = {}): FileDiff => ({
  path: "src/A.java",
  status: "modified",
  binary: false,
  additions: 1,
  deletions: 2,
  hunks: [{ header: "@@ -1 +1 @@", lines: [{ kind: "del", old: 1, text: "a" }, { kind: "add", new: 1, text: "b" }] }],
  truncated: false,
  ...over,
});

describe("diffview", () => {
  it("labels status and renames", () => {
    expect(statusLetter("renamed")).toBe("R");
    expect(fileLabel(file())).toBe("src/A.java");
    expect(fileLabel(file({ status: "renamed", old_path: "old/A.java" }))).toBe("old/A.java → src/A.java");
  });

  it("opens small files only", () => {
    expect(shownLines(file())).toBe(2);
    expect(startsOpen(file())).toBe(true);
    expect(startsOpen(file({ hunks: [], binary: true }))).toBe(false);
    const big = Array.from({ length: OPEN_MAX_LINES + 1 }, (_, i) => ({ kind: "add" as const, new: i + 1, text: "" }));
    expect(startsOpen(file({ hunks: [{ header: "@@", lines: big }] }))).toBe(false);
  });

  it("sums files and line counts", () => {
    expect(totals({ base: "a", head: "b", truncated: false, files: [file(), file({ additions: 5, deletions: 0 })] })).toEqual({
      files: 2,
      additions: 6,
      deletions: 2,
    });
  });

  it("shortens full SHAs in range labels", () => {
    const a = "a".repeat(40);
    const b = "b".repeat(40);
    expect(rangeLabel(a, b)).toBe("aaaaaaa..bbbbbbb");
    expect(rangeLabel(null, null)).toBe("working tree");
    expect(rangeLabel("main", null)).toBe("main..working tree");
  });
});
