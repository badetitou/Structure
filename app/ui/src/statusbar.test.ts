import { describe, expect, it } from "vitest";
import type { Impact } from "./api";
import type { ViewCell } from "./domaintree";
import {
  baseName,
  impactLabel,
  selectionLabel,
  shortRef,
  snapshotLabel,
  statsLabel,
} from "./statusbar";

const SHA = "0123456789abcdef0123456789abcdef01234567";
const impact = (over: Partial<Impact> = {}): Impact => ({
  base: SHA,
  head: null,
  changed_files: [],
  changed_symbols: [],
  changed_cells: ["a"],
  impacted_cells: [
    { id: "a", distance: 0 },
    { id: "b", distance: 1 },
  ],
  impacted_symbols: [],
  ...over,
});

describe("baseName", () => {
  it("keeps the last segment, ignoring trailing separators", () => {
    expect(baseName("/a/b/proj/")).toBe("proj");
    expect(baseName("C:\\x\\proj")).toBe("proj");
    expect(baseName("/")).toBe("/");
  });
});

describe("shortRef", () => {
  it("shortens full SHAs, keeps names, null is the working tree", () => {
    expect(shortRef(SHA)).toBe("01234567");
    expect(shortRef("HEAD~1")).toBe("HEAD~1");
    expect(shortRef(null)).toBe("working tree");
  });
});

describe("statsLabel", () => {
  it("formats counts and switches to seconds from 1000 ms", () => {
    expect(
      statsLabel({ files: 1234, symbols: 1, edges: 0, elapsed_ms: 999 }),
    ).toBe("1,234 files · 1 symbols · 0 edges · 999 ms");
    expect(
      statsLabel({ files: 1, symbols: 1, edges: 1, elapsed_ms: 1250 }),
    ).toMatch(/· 1\.3 s$/);
  });
});

describe("snapshotLabel", () => {
  it("names the commit only when its snapshot is shown", () => {
    expect(snapshotLabel(null)).toBe("working tree");
    expect(snapshotLabel(impact({ head: SHA }))).toBe("working tree");
    const m = { project: "p" } as Impact["matrix"];
    expect(snapshotLabel(impact({ head: SHA, matrix: m }))).toBe(
      "snapshot 01234567",
    );
  });
});

describe("impactLabel", () => {
  it("shows progress first, then the summary, else null", () => {
    expect(impactLabel(null, null)).toBeNull();
    expect(impactLabel(impact(), { step: "indexing", pct: 40 })).toBe(
      "Impact: indexing (40%)",
    );
    expect(impactLabel(impact(), null)).toBe(
      "Impact 01234567..working tree: 1 changed cell, 2 impacted cells",
    );
  });
});

describe("selectionLabel", () => {
  it("describes the selected cell", () => {
    expect(selectionLabel(null)).toBeNull();
    const c = {
      id: "x",
      layer: "ui",
      domain: "d",
      label: "d",
      size: 1,
      members: ["x"],
    } as ViewCell;
    expect(selectionLabel(c)).toBe("ui / d · 1 symbol");
  });
});
