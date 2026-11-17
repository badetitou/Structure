import { describe, expect, it } from "vitest";
import type { Impact, Matrix } from "./api";
import {
  MAX_BANDS,
  bandRects,
  cellOverlayRows,
  cellsNotIn,
  hitOpacity,
  hitsTitle,
  markerRect,
  multiCellPaint,
  overlayViews,
} from "./multioverlay";
import type { OverlayHit, OverlayRef } from "./multioverlay";

const imp = (changed: string[], impacted: [string, number][] = []): Impact => ({
  base: "a",
  head: null,
  changed_files: [],
  changed_symbols: [],
  changed_cells: changed,
  impacted_cells: impacted.map(([id, distance]) => ({ id, distance })),
  impacted_symbols: [],
});
const ov = (id: string, impact: Impact, colour = `--overlay-${id.slice(1)}`): OverlayRef => ({ id, label: `L${id}`, colour, impact });
const hit = (id: string, role: "changed" | "impacted", distance: number | null = null): OverlayHit => ({
  id,
  label: `L${id}`,
  colour: `--overlay-${id.slice(1)}`,
  role,
  distance,
});

describe("overlayViews", () => {
  // matrix cells a, b, c; a and b are merged into the view cell "ab" (a collapsed domain)
  const cellMap = new Map([
    ["a", "ab"],
    ["b", "ab"],
    ["c", "c"],
  ]);

  it("lists each cell's overlays in the overlays' order, changed beating impacted in one overlay", () => {
    const v = overlayViews(
      [
        ov("o0", imp(["a"], [["b", 2], ["c", 3]])),
        ov("o1", imp([], [["c", 1]])),
      ],
      cellMap,
    );
    expect(v.hits.get("ab")).toEqual([hit("o0", "changed")]);
    expect(v.hits.get("c")).toEqual([hit("o0", "impacted", 3), hit("o1", "impacted", 1)]);
  });

  it("the union is changed in any, impacted at the smallest distance", () => {
    const v = overlayViews([ov("o0", imp(["a"], [["c", 3]])), ov("o1", imp([], [["c", 1], ["a", 2]]))], cellMap);
    expect([...v.union.changed]).toEqual(["ab"]);
    expect(v.union.impacted.get("c")).toBe(1);
    // o1 impacts ab, o0 changes it: both hits are kept
    expect(v.hits.get("ab")?.map((h) => h.role)).toEqual(["changed", "impacted"]);
  });

  it("skips cells the view does not have", () => {
    const v = overlayViews([ov("o0", imp(["gone"], [["lost", 1]]))], cellMap);
    expect(v.hits.size).toBe(0);
    expect(v.union.changed.size).toBe(0);
  });
});

describe("cellsNotIn", () => {
  it("counts distinct hit cells missing from the matrix", () => {
    const m = { cells: [{ id: "a" }, { id: "b" }] } as unknown as Matrix;
    expect(cellsNotIn([imp(["a", "x"], [["y", 1]]), imp(["x"], [["b", 1]])], m)).toBe(2);
    expect(cellsNotIn([imp(["a"])], m)).toBe(0);
    expect(cellsNotIn([], m)).toBe(0);
  });
});

describe("multiCellPaint", () => {
  it("dims a cell hit by no overlay", () => {
    expect(multiCellPaint(undefined)).toEqual({ kind: "none" });
    expect(multiCellPaint([])).toEqual({ kind: "none" });
  });

  it("one band per overlay up to three, in their colours", () => {
    const p = multiCellPaint([hit("o0", "changed"), hit("o2", "impacted", 1)]);
    expect(p).toEqual({
      kind: "bands",
      changed: true,
      bands: [
        { colour: "var(--overlay-0)", opacity: 1, changed: true },
        { colour: "var(--overlay-2)", opacity: hitOpacity(hit("o2", "impacted", 1)), changed: false },
      ],
    });
    const three = multiCellPaint([hit("o0", "impacted", 1), hit("o1", "impacted", 2), hit("o2", "impacted", 3)]);
    expect(three.kind === "bands" && three.bands.length).toBe(MAX_BANDS);
    expect(three.kind === "bands" && three.changed).toBe(false);
  });

  it("above three, the neutral multiple style", () => {
    const hits = [hit("o0", "impacted", 1), hit("o1", "impacted", 1), hit("o2", "changed"), hit("o3", "impacted", 4)];
    expect(multiCellPaint(hits)).toEqual({ kind: "multiple", count: 4, changed: true });
  });

  it("changed is solid; impacted fades with the distance and stays well under it", () => {
    expect(hitOpacity(hit("o0", "changed"))).toBe(1);
    const ops = [1, 2, 3, 4, 5, 9, Infinity].map((d) => hitOpacity(hit("o0", "impacted", d)));
    expect(ops[0]).toBeLessThanOrEqual(0.6);
    for (let i = 1; i < 5; i++) expect(ops[i]).toBeLessThan(ops[i - 1]);
    expect(ops[5]).toBe(ops[4]);
    expect(ops[6]).toBe(ops[4]);
  });
});

describe("band geometry", () => {
  it("splits a tile into k equal side-by-side bands covering it", () => {
    const r = { x: 10, y: 20, w: 30, h: 12 };
    const bs = bandRects(r, 3);
    expect(bs).toEqual([
      { x: 10, y: 20, w: 10, h: 12 },
      { x: 20, y: 20, w: 10, h: 12 },
      { x: 30, y: 20, w: 10, h: 12 },
    ]);
    expect(bandRects(r, 1)).toEqual([r]);
    expect(bandRects(r, 0)).toEqual([]);
  });

  it("the changed marker is a strip along the band's top", () => {
    expect(markerRect({ x: 1, y: 2, w: 10, h: 40 })).toEqual({ x: 1, y: 2, w: 10, h: 3 });
    expect(markerRect({ x: 1, y: 2, w: 10, h: 4 })).toEqual({ x: 1, y: 2, w: 10, h: 1 });
  });
});

describe("cell texts", () => {
  it("the tooltip lists the overlays hitting the cell with their role and distance", () => {
    expect(hitsTitle("svc/orders (3)", undefined)).toBe("svc/orders (3)");
    expect(hitsTitle("svc/orders (3)", [hit("o0", "changed"), hit("o1", "impacted", 2)])).toBe(
      "svc/orders (3)\n• Lo0 — changed\n• Lo1 — impacted, distance 2",
    );
  });

  it("the details list every overlay shown, hit or not", () => {
    const rows = cellOverlayRows([ov("o0", imp([])), ov("o1", imp([])), ov("o2", imp([]))], [hit("o2", "impacted", 1), hit("o0", "changed")]);
    expect(rows.map((r) => [r.id, r.hit, r.text])).toEqual([
      ["o0", true, "changed"],
      ["o1", false, "not hit"],
      ["o2", true, "impacted, distance 1"],
    ]);
  });
});
