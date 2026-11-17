import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Matrix } from "../api";

vi.mock("../api", () => ({
  capabilities: vi.fn(),
  analyze: vi.fn(),
  reclassify: vi.fn(),
  rulesStamp: vi.fn(),
  impact: vi.fn(),
  cellSymbols: vi.fn(),
  lenses: vi.fn(),
  matrix: vi.fn(),
  impactCells: vi.fn(),
  history: vi.fn(),
  diff: vi.fn(),
}));

import { DEFAULT_LENSES } from "../lens";
import { createViewContext } from "../workspace/context";
import { lenses } from "./lenses.svelte";
import { selection } from "./selection.svelte";
import { reloadRules, runImpact } from "./session";
import { cell, flush, imp, loaded, mocked, page, setup, withCells } from "./testing";

const both = [...DEFAULT_LENSES, { id: "layer-package", label: "Layer × package" }];
const packaged = (generation: number): Matrix => withCells(generation, ["package/service:com.x", 3]);
const onPackages = { display: { lens: "layer-package" } };

beforeEach(() => {
  setup();
  mocked.lenses.mockResolvedValue(both);
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
});
afterEach(() => vi.unstubAllGlobals());

describe("lens store", () => {
  it("lists the lenses the backend reports once a project is loaded", async () => {
    expect(lenses.available).toEqual(DEFAULT_LENSES);
    await loaded();
    await flush();
    expect(lenses.available).toEqual(both);
  });

  it("gives the matrix itself for the default lens and fetches another lens once", async () => {
    const work = withCells(7, ["service:a", 3]);
    await loaded(work);
    expect(lenses.matrix("layer-domain", work, null)).toBe(work);
    expect(lenses.matrix("layer-package", null, null)).toBeNull();
    const p = packaged(7);
    mocked.matrix.mockResolvedValue(p);
    expect(lenses.matrix("layer-package", work, null)).toBeNull();
    await flush();
    expect(mocked.matrix).toHaveBeenCalledWith(["/p"], null, 7, "layer-package");
    expect(lenses.matrix("layer-package", work, null)).toBe(p);
    expect(lenses.matrix("layer-package", work, null)).toBe(p);
    await flush();
    expect(mocked.matrix).toHaveBeenCalledTimes(1);
    // Another generation (reclassified) or commit is another matrix.
    lenses.matrix("layer-package", withCells(8), null);
    lenses.matrix("layer-package", work, "abc");
    await flush();
    expect(mocked.matrix).toHaveBeenCalledTimes(3);
    expect(mocked.matrix).toHaveBeenLastCalledWith(["/p"], "abc", 7, "layer-package");
  });

  it("reports a lens matrix that failed", async () => {
    const work = withCells(7);
    await loaded(work);
    mocked.matrix.mockRejectedValue(new Error("boom"));
    expect(lenses.matrixError("layer-package", work, null)).toBe("");
    await flush();
    expect(lenses.matrix("layer-package", work, null)).toBeNull();
    expect(lenses.matrixError("layer-package", work, null)).toContain("boom");
  });

  it("maps an impact's symbols to a lens' cells, stable objects, none while loading", async () => {
    await loaded();
    const i = imp({ head: "c2", changed_symbols: ["S"], changed_cells: ["service:a"], impacted_symbols: [{ id: "T", distance: 2 }] });
    expect(lenses.impact("layer-domain", i)).toBe(i);
    mocked.impactCells.mockResolvedValue({ changed_cells: ["package/service:p"], impacted_cells: [] });
    const pending = lenses.impact("layer-package", i);
    expect(pending.changed_cells).toEqual([]);
    expect(lenses.impact("layer-package", i)).toBe(pending);
    await flush();
    expect(mocked.impactCells).toHaveBeenCalledWith("/p", "c2", "layer-package", ["S"], [{ id: "T", distance: 2 }]);
    const done = lenses.impact("layer-package", i);
    expect(done.changed_cells).toEqual(["package/service:p"]);
    expect(done.changed_symbols).toEqual(["S"]);
    expect(lenses.impact("layer-package", i)).toBe(done);
  });
});

describe("view context, per panel lens", () => {
  it("draws a panel through its lens: matrix and overlays; the others keep theirs", async () => {
    const ctx = createViewContext();
    const work = withCells(7, ["service:a", 3]);
    await loaded(work);
    await flush();
    const p = packaged(7);
    mocked.matrix.mockResolvedValue(p);
    const pa = ctx.forPanel("matrix", onPackages, () => {});
    const pb = ctx.forPanel("matrix-2", undefined, () => {});
    expect([pa.lens.id, pb.lens.id]).toEqual(["layer-package", "layer-domain"]);
    expect(pa.lens.label).toBe("Layer × package");
    expect(pb.lens.matrix).toBe(work);
    expect(pa.lens.matrix).toBeNull();
    await flush();
    expect(pa.lens.matrix).toBe(p);

    mocked.impact.mockResolvedValue(imp({ changed_symbols: ["S"], changed_cells: ["service:a"] }));
    mocked.impactCells.mockResolvedValue({ changed_cells: ["package/service:com.x"], impacted_cells: [] });
    await runImpact(null, null);
    void pa.overlays.current;
    await flush();
    expect(pa.overlays.current?.changed_cells).toEqual(["package/service:com.x"]);
    expect(pa.overlays.selected?.impact.changed_cells).toEqual(["package/service:com.x"]);
    expect(pb.overlays.current?.changed_cells).toEqual(["service:a"]);
    // The base context (toolbar, details) follows the active panel's lens.
    ctx.displays.focus("matrix");
    expect(ctx.lens.id).toBe("layer-package");
    expect(ctx.lens.matrix).toBe(p);
    ctx.displays.focus("matrix-2");
    expect(ctx.lens.id).toBe("layer-domain");
    expect(ctx.overlays.current?.changed_cells).toEqual(["service:a"]);
    // The toolbar switches the active panel's lens.
    ctx.actions.setLens("layer-package");
    expect(pb.lens.id).toBe("layer-package");
  });

  it("falls back to the default lens when the backend does not offer the panel's", async () => {
    mocked.lenses.mockResolvedValue([...DEFAULT_LENSES]);
    const ctx = createViewContext();
    const work = withCells(7);
    await loaded(work);
    await flush();
    const pa = ctx.forPanel("matrix", onPackages, () => {});
    expect(pa.lens.id).toBe("layer-domain");
    expect(pa.lens.matrix).toBe(work);
    await flush();
    expect(mocked.matrix).not.toHaveBeenCalled();
  });

  it("keeps the selection's lens: listed in it, not dropped by a panel of another lens", async () => {
    const ctx = createViewContext();
    await loaded(withCells(7, ["service:a", 3]));
    await flush();
    mocked.matrix.mockResolvedValue(packaged(7));
    const pa = ctx.forPanel("matrix", onPackages, () => {});
    const pb = ctx.forPanel("matrix-2", undefined, () => {});
    mocked.cellSymbols.mockResolvedValue(page);
    pa.actions.selectCell(cell);
    expect(ctx.selection.lens).toBe("layer-package");
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], null, 7, cell.members, 0, 500, "layer-package");
    // A panel of the default lens, even active, leaves it alone.
    ctx.displays.focus("matrix-2");
    pb.actions.syncSelection([]);
    expect(selection.selected).toBe(cell);
    // Its own panel keeps it in step once active.
    ctx.displays.focus("matrix");
    pa.actions.syncSelection([]);
    expect(selection.selected).toBeNull();
    // Selected in the default lens: listed as before, without a lens argument.
    pb.actions.selectCell(cell);
    expect(ctx.selection.lens).toBe("layer-domain");
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], null, 7, cell.members, 0, 500);
  });
  it("carries a selection of another lens over a rules reload, in its lens", async () => {
    await loaded(withCells(7, ["service:a", 3]));
    await flush();
    mocked.cellSymbols.mockResolvedValue(page);
    const pcell = { ...cell, id: "p1", members: ["package/service:com.x"], size: 3 } as typeof cell;
    selection.select(pcell, "layer-package");
    // The lens' new matrix not loaded yet: kept as is, in its lens.
    mocked.reclassify.mockResolvedValue(withCells(8, ["service:b", 4]));
    await reloadRules();
    expect(selection.selected).toBe(pcell);
    expect(selection.lens).toBe("layer-package");
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], null, 8, pcell.members, 0, 500, "layer-package");
    // Loaded: retained against it.
    mocked.matrix.mockResolvedValue(withCells(9, ["package/service:com.x", 5]));
    lenses.matrix("layer-package", withCells(9), null);
    await flush();
    mocked.reclassify.mockResolvedValue(withCells(9, ["service:b", 4]));
    await reloadRules();
    expect(selection.selected?.size).toBe(5);
    expect(selection.lens).toBe("layer-package");
    // Gone from it: dropped.
    mocked.matrix.mockResolvedValue(withCells(10, ["package/service:com.y", 1]));
    lenses.matrix("layer-package", withCells(10), null);
    await flush();
    mocked.reclassify.mockResolvedValue(withCells(10, ["service:b", 4]));
    await reloadRules();
    expect(selection.selected).toBeNull();
  });
});
