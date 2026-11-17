import { describe, expect, it } from "vitest";
import type { Impact } from "./api";
import type { ViewCell } from "./domaintree";
import { cellCountsLabel, impactLabel, multiImpactLabel, selectionLabel, shortRef, snapshotLabel, statsLabel } from "./statusbar";

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
const cell = (size: number) => ({ id: "x", layer: "ui", domain: "d", label: "d", size, members: ["x"] }) as ViewCell;

describe("shortRef", () => {
  it("shortens full SHAs (any case) like the diff panel, keeps names, null is the working tree", () => {
    expect(shortRef(SHA)).toBe("0123456");
    expect(shortRef(SHA.toUpperCase())).toBe("0123456");
    expect(shortRef(SHA.slice(1))).toBe(SHA.slice(1)); // 39 hex chars: not a full SHA
    expect(shortRef("HEAD~1")).toBe("HEAD~1");
    expect(shortRef(null)).toBe("working tree");
  });
});

describe("statsLabel", () => {
  it("formats counts and switches to seconds at 1000 ms", () => {
    expect(statsLabel({ files: 1234, symbols: 1, edges: 0, elapsed_ms: 0 })).toBe("1,234 files · 1 symbols · 0 edges · 0 ms");
    expect(statsLabel({ files: 1, symbols: 1, edges: 1, elapsed_ms: 999 })).toBe("1 files · 1 symbols · 1 edges · 999 ms");
    expect(statsLabel({ files: 1, symbols: 1, edges: 1, elapsed_ms: 1000 })).toBe("1 files · 1 symbols · 1 edges · 1.0 s");
    expect(statsLabel({ files: 1, symbols: 1, edges: 1, elapsed_ms: 1250 })).toBe("1 files · 1 symbols · 1 edges · 1.3 s");
  });
});

describe("snapshotLabel", () => {
  it("names the commit only when its snapshot is shown", () => {
    expect(snapshotLabel(null)).toBe("working tree");
    expect(snapshotLabel(impact({ head: SHA }))).toBe("working tree");
    const m = { project: "p" } as Impact["matrix"];
    expect(snapshotLabel(impact({ head: SHA, matrix: m }))).toBe("snapshot 0123456");
  });
});

describe("impactLabel", () => {
  it("shows progress first, then the summary, else null", () => {
    expect(impactLabel(null, null)).toBeNull();
    expect(impactLabel(impact(), { step: "indexing", pct: 40 })).toBe("Impact: indexing (40%)");
    expect(impactLabel(impact(), null)).toBe("Impact 0123456..working tree: 1 changed cell, 2 impacted cells");
  });
  it("pluralises zero and keeps ref names", () => {
    expect(impactLabel(impact({ base: "main", head: SHA, changed_cells: [], impacted_cells: [] }), null)).toBe(
      "Impact main..0123456: 0 changed cells, 0 impacted cells",
    );
  });
});

describe("selectionLabel", () => {
  it("describes the selected cell", () => {
    expect(selectionLabel(null)).toBeNull();
    expect(selectionLabel(cell(1))).toBe("ui / d · 1 symbol");
    expect(selectionLabel(cell(12))).toBe("ui / d · 12 symbols");
  });
  it("uses the layer label of the matrix definitions", () => {
    expect(selectionLabel(cell(2), [{ id: "ui", label: "User interface", order: 0 }])).toBe(
      "User interface / d · 2 symbols",
    );
  });
});

describe("multiImpactLabel", () => {
  it("counts the overlays drawn and the cells the working tree does not have", () => {
    expect(multiImpactLabel(2, 0)).toBe("2 overlays on the working tree");
    expect(multiImpactLabel(3, 1)).toBe("3 overlays on the working tree · 1 cell not in working tree");
    expect(multiImpactLabel(3, 4)).toBe("3 overlays on the working tree · 4 cells not in working tree");
  });

  it("cell counts of one overlay", () => {
    expect(cellCountsLabel(impact())).toBe("1 changed cell · 2 impacted cells");
  });
});
