import { describe, expect, it } from "vitest";
import type { Impact, Matrix } from "./api";
import {
  DEFAULT_LENS,
  DEFAULT_LENSES,
  HeldMatrix,
  lensLabel,
  lensSelectorVisible,
  liveSnapshots,
  parseLens,
  resolveLens,
  snapshotId,
  withLensCells,
} from "./lens";

const both = [...DEFAULT_LENSES, { id: "layer-package", label: "Layer × package" }];

describe("lenses", () => {
  it("default to layer × domain", () => {
    expect(DEFAULT_LENS).toBe("layer-domain");
    expect(DEFAULT_LENSES.map((l) => l.id)).toEqual([DEFAULT_LENS]);
  });

  it("parse a stored id: any non-empty string, else the default lens", () => {
    expect(parseLens("layer-package")).toBe("layer-package");
    for (const v of [undefined, null, "", 3, {}]) expect(parseLens(v)).toBe(DEFAULT_LENS);
  });

  it("resolve to the default lens when the backend does not offer the one asked", () => {
    expect(resolveLens("layer-package", both)).toBe("layer-package");
    expect(resolveLens("layer-package", DEFAULT_LENSES)).toBe(DEFAULT_LENS);
    expect(resolveLens("gone", both)).toBe(DEFAULT_LENS);
  });

  it("keep the lens asked while the backend's list is not known (no default lens first)", () => {
    expect(resolveLens("layer-package", DEFAULT_LENSES, false)).toBe("layer-package");
    expect(resolveLens("layer-package", DEFAULT_LENSES, true)).toBe(DEFAULT_LENS);
  });

  it("show the selector only when there is a choice", () => {
    expect(lensSelectorVisible([])).toBe(false);
    expect(lensSelectorVisible(DEFAULT_LENSES)).toBe(false);
    expect(lensSelectorVisible(both)).toBe(true);
  });

  it("label a lens, the id when unknown", () => {
    expect(lensLabel("layer-package", both)).toBe("Layer × package");
    expect(lensLabel("gone", both)).toBe("gone");
  });

  it("swap an impact's cells for another lens' (none while unknown)", () => {
    const imp: Impact = {
      base: "a",
      head: null,
      changed_files: ["f"],
      changed_symbols: ["S"],
      changed_cells: ["b:x"],
      impacted_cells: [{ id: "b:y", distance: 1 }],
      impacted_symbols: [{ id: "T", distance: 1 }],
    };
    const cells = { changed_cells: ["package/b:p"], impacted_cells: [{ id: "package/b:q", distance: 1 }] };
    expect(withLensCells(imp, cells)).toEqual({ ...imp, ...cells });
    expect(withLensCells(imp, null)).toEqual({ ...imp, changed_cells: [], impacted_cells: [] });
    expect(imp.changed_cells).toEqual(["b:x"]);
  });

  it("name the analyses still shown: the working tree's and the overlays' snapshots", () => {
    const m = (generation: number) => ({ generation }) as Matrix;
    const imp = (head: string | null, matrix?: Matrix) => ({ head, matrix }) as Impact;
    const live = liveSnapshots(m(7), [imp("c1", m(9)), imp(null), imp("c2")]);
    expect([...live].sort()).toEqual([snapshotId(null, 7), snapshotId("c1", 9)].sort());
    expect(snapshotId("c1", 9)).not.toBe(snapshotId(null, 9));
    expect(liveSnapshots(null, []).size).toBe(0);
  });

  it("hold the last matrix of a lens while its next one loads, not on an error or another lens", () => {
    const a = { generation: 1 } as Matrix;
    const b = { generation: 2 } as Matrix;
    const h = new HeldMatrix();
    expect(h.show("p", null, "", "x")).toEqual({ matrix: null, loading: true });
    expect(h.show("p", a, "", "x")).toEqual({ matrix: a, loading: false });
    expect(h.show("p", null, "", "x")).toEqual({ matrix: a, loading: true });
    expect(h.show("p", b, "", "x")).toEqual({ matrix: b, loading: false });
    expect(h.show("q", null, "", "x")).toEqual({ matrix: null, loading: true });
    expect(h.show("p", null, "", "x")).toEqual({ matrix: null, loading: true });
    h.show("p", a, "", "x");
    expect(h.show("p", null, "boom", "x")).toEqual({ matrix: null, loading: false });
    expect(h.show("p", null, "", "x")).toEqual({ matrix: null, loading: true });
  });

  it("drop the held matrix when the project closes or changes, and never load without one", () => {
    const a = { generation: 1 } as Matrix;
    const h = new HeldMatrix();
    // No project analysed (closed, failed, or a saved lens before any load): no "Loading".
    expect(h.show("p", null, "", null)).toEqual({ matrix: null, loading: false });
    h.show("p", a, "", "x");
    expect(h.show("p", null, "", null)).toEqual({ matrix: null, loading: false });
    expect(h.show("p", null, "", "x")).toEqual({ matrix: null, loading: true });
    // Another project analysed while the panel was not redrawn in between.
    h.show("p", a, "", "x");
    expect(h.show("p", null, "", "y")).toEqual({ matrix: null, loading: true });
  });
});
