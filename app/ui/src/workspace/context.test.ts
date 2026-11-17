import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ViewMatrix } from "../domaintree";

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

import { analysis } from "../stores/analysis.svelte";
import { git } from "../stores/git.svelte";
import { impact } from "../stores/impact.svelte";
import { options } from "../stores/options.svelte";
import { selection } from "../stores/selection.svelte";
import { runImpact } from "../stores/session";
import { shown } from "../stores/shown";
import { allCaps, cell, flush, imp, loaded, mocked, mx, setup } from "../stores/testing";
import { createViewContext } from "./context";

beforeEach(() => {
  setup();
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
});
afterEach(() => vi.unstubAllGlobals());

describe("view context, read side", () => {
  it("reflects the analysis and the matrix shown", async () => {
    const ctx = createViewContext();
    expect(ctx.lens.matrix).toBeNull();
    const work = mx("work", ["w"]);
    await loaded(work);
    expect(ctx.lens.paths).toEqual(["/p"]);
    expect(ctx.lens.project).toBe("/p");
    expect(ctx.lens.multi).toBe(false);
    expect(ctx.lens.work).toBe(work);
    expect(ctx.lens.matrix).toBe(work);
    expect(ctx.lens.head).toBeNull();
    expect(ctx.lens.warnings).toEqual(["w"]);
    expect(ctx.lens.missingCells).toBe(0);
    const snap = mx("snap");
    mocked.impact.mockResolvedValue(imp({ head: "abc", matrix: snap }));
    await runImpact("old", "abc");
    expect(ctx.lens.matrix).toBe(shown.matrix);
    expect(ctx.lens.matrix).toBe(snap);
    expect(ctx.lens.head).toBe("abc");
  });

  it("reflects the overlays, the selection, git, rules, options and capabilities", async () => {
    const ctx = createViewContext();
    await loaded();
    expect(ctx.overlays.list).toEqual([]);
    expect(ctx.overlays.selected).toBeNull();
    mocked.impact.mockResolvedValue(imp());
    await runImpact(null, null);
    expect(ctx.overlays.list).toBe(impact.overlays);
    expect(ctx.overlays.selectedIds).toEqual(impact.selectedIds);
    expect(ctx.overlays.selected).toBe(impact.selected);
    expect(ctx.overlays.selectedOverlays).toEqual(impact.selectedOverlays);
    expect(ctx.overlays.current).toBe(impact.current);
    expect(ctx.overlays.range).toEqual({ base: null, head: null });
    expect(ctx.overlays.multi).toBe(false);
    expect(ctx.overlays.busy).toBe(false);
    expect(ctx.overlays.progress).toBeNull();
    expect(ctx.overlays.error).toBe("");

    mocked.cellSymbols.mockResolvedValue({ total: 2, offset: 0, symbols: [] });
    selection.select(cell);
    expect(ctx.selection.selected).toBe(cell);
    expect(ctx.selection.symbols).toBe(selection.symbols);
    expect(ctx.selection.view).toBeNull();

    expect(ctx.git.history).toBe(git.history);
    expect(ctx.git.historySel).toBeNull();
    expect(ctx.git.diff).toBeNull();
    expect(ctx.git.historyAvailable).toBe(true);
    expect(ctx.git.diffAvailable).toBe(true);

    expect(ctx.rules.reloadable).toBe(analysis.rulesReloadable);
    expect(ctx.rules.busy).toBe(false);
    expect(ctx.rules.error).toBe("");

    expect(ctx.lens.display.mode).toBe(options.mode);
    expect(ctx.lens.display.transposed).toBe(options.transposed);
    expect(ctx.capabilities).toEqual(allCaps);
  });
});

describe("view context, actions", () => {
  it("select a cell, page its symbols and keep the drawn view", async () => {
    const ctx = createViewContext();
    await loaded();
    const sym = (name: string) => ({ id: name, name, file: "f" });
    mocked.cellSymbols.mockResolvedValueOnce({ total: 2, offset: 0, symbols: [sym("a")] });
    ctx.actions.selectCell(cell);
    await flush();
    expect(selection.selected).toBe(cell);
    expect(ctx.selection.symbols?.items.map((s) => s.name)).toEqual(["a"]);
    mocked.cellSymbols.mockResolvedValueOnce({ total: 2, offset: 1, symbols: [sym("b")] });
    await ctx.actions.loadMoreSymbols();
    expect(ctx.selection.symbols?.items.map((s) => s.name)).toEqual(["a", "b"]);

    const vm = { cells: [] } as unknown as ViewMatrix;
    ctx.actions.publishView(vm);
    expect(selection.view).toBe(vm);
    ctx.actions.syncSelection([]);
    expect(selection.selected).toBeNull();
    ctx.actions.publishView(null);
    expect(selection.view).toBeNull();
  });

  it("run, select, keep, remove and clear overlays through the session", async () => {
    const ctx = createViewContext();
    await loaded();
    mocked.impact.mockResolvedValue(imp());
    mocked.diff.mockResolvedValue({ base: "HEAD", head: "worktree", files: [], truncated: false });
    ctx.actions.runImpact("a", null);
    await flush();
    expect(impact.overlays).toHaveLength(1);
    expect(git.diff?.base).toBe("a");
    ctx.actions.keepOverlays();
    expect(impact.overlays[0]?.pinned).toBe(true);
    ctx.actions.runImpact("b", null);
    await flush();
    expect(impact.overlays).toHaveLength(2);
    const [first, second] = impact.overlays.map((o) => o.id);
    ctx.actions.selectOverlay(first, true);
    expect(impact.selectedIds).toEqual([second, first]);
    ctx.actions.selectOverlay(first);
    expect(impact.selectedIds).toEqual([first]);
    ctx.actions.removeOverlay(second);
    expect(impact.overlays.map((o) => o.id)).toEqual([first]);
    ctx.actions.clearImpact();
    expect(impact.selectedIds).toEqual([]);
    expect(git.diff).toBeNull();
  });

  it("pick a history entry", async () => {
    const ctx = createViewContext();
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits: [] });
    await analysis.load(["/p"]);
    await git.loadHistory("/p", () => true);
    mocked.impact.mockResolvedValue(imp());
    mocked.diff.mockResolvedValue({ base: "HEAD", head: "worktree", files: [], truncated: false });
    ctx.actions.pickHistory({ kind: "worktree" });
    expect(git.historySel).toEqual({ kind: "worktree" });
    await flush();
    expect(impact.overlays).toHaveLength(1);
  });

  it("reload the rules", async () => {
    const ctx = createViewContext();
    await loaded();
    const next = mx("next");
    mocked.rulesStamp.mockResolvedValue("s");
    mocked.reclassify.mockResolvedValue(next);
    await ctx.actions.reloadRules();
    expect(analysis.work).toBe(next);
  });

  it("set the display options", () => {
    const ctx = createViewContext();
    ctx.actions.setMode("dense");
    expect(ctx.lens.display.mode).toBe("dense");
    for (const on of [true, false]) {
      ctx.actions.setTransposed(on);
      ctx.actions.setAligned(on);
      ctx.actions.setShowHttp(on);
      ctx.actions.setShowRpc(on);
      ctx.actions.setAngledLabels(on);
      expect(options).toMatchObject({ transposed: on, aligned: on, showHttp: on, showRpc: on, angledLabels: on });
    }
    ctx.actions.setMode("normal");
    expect(options.mode).toBe("normal");
  });
});

describe("view context, per panel display", () => {
  const persisted = () => {
    const writes: unknown[] = [];
    return { writes, persist: (d: unknown) => void writes.push(d) };
  };

  it("give each matrix panel its own options; the base context follows the active one", () => {
    const ctx = createViewContext();
    options.setMode("normal");
    options.setTransposed(false);
    options.setShowHttp(true);
    expect(ctx.panel).toBeNull();
    const a = persisted();
    const b = persisted();
    const pa = ctx.forPanel("matrix", undefined, a.persist);
    const pb = ctx.forPanel("matrix-2", { display: { mode: "dense", transposed: true } }, b.persist);
    expect(pa.panel).toBe("matrix");
    expect(pb.panel).toBe("matrix-2");
    // Migration: the first panel adopts the global options and stores them once.
    expect(a.writes).toEqual([{ mode: "normal", transposed: false, aligned: options.aligned, showHttp: options.showHttp, showRpc: options.showRpc, lens: "layer-domain" }]);
    expect(b.writes).toEqual([]);
    expect([pa.lens.display.mode, pb.lens.display.mode]).toEqual(["normal", "dense"]);
    expect([pa.lens.display.transposed, pb.lens.display.transposed]).toEqual([false, true]);

    pb.actions.setMode("flame");
    expect([pa.lens.display.mode, pb.lens.display.mode]).toEqual(["normal", "flame"]);
    expect(b.writes.at(-1)).toMatchObject({ mode: "flame", transposed: true });
    // The global defaults keep the last choice (a new view or a reset layout starts from it).
    expect(options.mode).toBe("flame");

    // The base context (toolbar, other panels) reads and sets the active panel's.
    expect(ctx.displays.activeId).toBe("matrix");
    expect(ctx.lens.display.mode).toBe("normal");
    ctx.actions.setTransposed(true);
    expect(pa.lens.display.transposed).toBe(true);
    ctx.displays.focus("matrix-2");
    expect(ctx.lens.display.mode).toBe("flame");
    ctx.actions.setShowHttp(false);
    expect([pa.lens.display.showHttp, pb.lens.display.showHttp]).toEqual([true, false]);

    // Angled labels stay app-wide.
    pa.actions.setAngledLabels(true);
    expect([ctx.lens.display.angledLabels, pb.lens.display.angledLabels]).toEqual([true, true]);
    pa.actions.setAngledLabels(false);

    // No matrix panel left: the global options.
    ctx.displays.drop("matrix");
    ctx.displays.drop("matrix-2");
    expect(ctx.lens.display.mode).toBe(options.mode);
    ctx.actions.setMode("dense");
    expect(options.mode).toBe("dense");
    ctx.actions.setMode("normal");
  });

  it("share the selection and overlays, the active panel naming the cells", async () => {
    const ctx = createViewContext();
    await loaded();
    const pa = ctx.forPanel("matrix", undefined, () => {});
    const pb = ctx.forPanel("matrix-2", undefined, () => {});
    // Each panel reads the overlays through its own lens; on the default lens, the store's own.
    mocked.impact.mockResolvedValue(imp());
    await runImpact(null, null);
    expect(pa.overlays.list).toBe(ctx.overlays.list);
    expect(pa.overlays.selectedIds).toBe(ctx.overlays.selectedIds);
    expect(pa.overlays.current).toBe(ctx.overlays.current);
    expect(pb.selection).toBe(ctx.selection);
    const va = { cells: [cell] } as unknown as ViewMatrix;
    const vb = { cells: [] } as unknown as ViewMatrix;
    pa.actions.publishView(va);
    pb.actions.publishView(vb);
    expect(ctx.selection.view).toBe(va);
    mocked.cellSymbols.mockResolvedValue({ total: 2, offset: 0, symbols: [] });
    pb.actions.selectCell(cell);
    expect(ctx.selection.selected).toBe(cell);
    // Only the active panel keeps the selection in step with its cells.
    pb.actions.syncSelection([]);
    expect(ctx.selection.selected).toBe(cell);
    ctx.displays.focus("matrix-2");
    expect(pa.selection.view).toBe(vb);
    pb.actions.syncSelection([]);
    expect(ctx.selection.selected).toBeNull();
    pb.actions.publishView(null);
    expect(ctx.selection.view).toBeNull();
  });
});
