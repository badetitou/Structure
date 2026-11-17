import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Impact } from "../api";

vi.mock("../api", () => ({
  capabilities: vi.fn(),
  analyze: vi.fn(),
  reclassify: vi.fn(),
  rulesStamp: vi.fn(),
  impact: vi.fn(),
  cellSymbols: vi.fn(),
  history: vi.fn(),
  diff: vi.fn(),
}));

import { ImpactOverlays, impact } from "./impact.svelte";
import { selection } from "./selection.svelte";
import { git } from "./git.svelte";
import { shown } from "./shown";
import { load, runImpact, clearImpact, pickHistory, runTypedImpact, selectOverlay, keepOverlay, removeOverlay, reloadRules } from "./session";
import { cell, flush, imp, loaded, mocked, mx, setup, withCells } from "./testing";

beforeEach(setup);

describe("impact", () => {
  it("shows the snapshot matrix of an impact, and its warnings", async () => {
    const work = mx("work", ["w"]);
    const snap = mx("snap");
    await loaded(work);
    expect(shown.warnings).toEqual(["w"]);
    mocked.impact.mockResolvedValue(imp({ head: "abc", matrix: snap, warnings: ["x"] }));
    await runImpact("old", "abc");
    expect(shown.matrix).toBe(snap);
    expect(shown.warnings).toEqual(["x"]);
    clearImpact();
    expect(shown.matrix).toBe(work);
    expect(impact.current).toBeNull();
  });

  it("drops the selection only when the shown matrix changes", async () => {
    await loaded();
    mocked.cellSymbols.mockResolvedValue({ total: 2, offset: 0, symbols: [] });
    selection.select(cell);
    mocked.impact.mockResolvedValue(imp());
    await runImpact(null, null);
    expect(selection.selected).toBe(cell);
    mocked.impact.mockResolvedValue(imp({ head: "h", matrix: mx("snap") }));
    await runImpact("a", "h");
    expect(selection.selected).toBeNull();
  });

  it("ignores an impact answer superseded by Clear", async () => {
    await loaded();
    let resolve: (i: Impact) => void = () => {};
    mocked.impact.mockReturnValue(new Promise((r) => (resolve = r)));
    const run = runImpact(null, null);
    expect(impact.busy).toBe(true);
    clearImpact();
    resolve(imp());
    await run;
    expect(impact.current).toBeNull();
    expect(impact.busy).toBe(false);
  });

  it("history selection runs the matching impact; a typed diff clears the selection", async () => {
    const commits = [
      { id: "c2", short: "c2", parents: ["c1"], base: "c1", subject: "s", author: "a", date: "", refs: [] },
      { id: "c1", short: "c1", parents: [], base: "empty", subject: "s", author: "a", date: "", refs: [] },
    ];
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: false, commits });
    mocked.impact.mockResolvedValue(imp());
    await load("/p");
    pickHistory({ kind: "range", newer: 0, older: 1, anchor: 0 });
    expect(mocked.impact).toHaveBeenCalledWith("/p", "empty", "c2", expect.any(Function));
    expect(git.historySel?.kind).toBe("range");
    await flush();
    runTypedImpact("x", null);
    expect(git.historySel).toBeNull();
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "x", null, expect.any(Function));
  });

  it("ignores new diffs while an impact runs and reports its progress", async () => {
    const commits = [{ id: "c1", short: "c1", parents: [], base: "e", subject: "s", author: "a", date: "", refs: [] }];
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits });
    await load("/p");
    mocked.impact.mockClear();
    let resolve: (i: Impact) => void = () => {};
    let report: (p: { step: string; pct: number }) => void = () => {};
    mocked.impact.mockImplementationOnce((_p, _b, _h, onProgress) => {
      report = onProgress ?? report;
      return new Promise((r) => (resolve = r));
    });
    pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    expect(impact.busy).toBe(true);
    expect(impact.progress?.pct).toBe(0);
    report({ step: "parsing", pct: 40 });
    expect(impact.progress).toEqual({ step: "parsing", pct: 40 });

    pickHistory({ kind: "worktree" });
    runTypedImpact("x", null);
    await runImpact("c", "d");
    expect(mocked.impact).toHaveBeenCalledTimes(1);
    expect(git.historySel).toEqual({ kind: "range", newer: 0, older: 0, anchor: 0 });

    resolve(imp());
    await flush();
    expect(impact.busy).toBe(false);
    expect(impact.progress).toBeNull();
  });

  it("an impact failure is reported and cleared by Clear", async () => {
    await loaded();
    mocked.impact.mockRejectedValueOnce(new Error("bad ref"));
    await runImpact("c", "d");
    expect(impact.busy).toBe(false);
    expect(impact.progress).toBeNull();
    expect(impact.error).toContain("bad ref");
    clearImpact();
    expect(impact.error).toBe("");
  });
});

describe("impact overlays", () => {
  it("one impact at a time: the next one replaces it, no selector", async () => {
    await loaded();
    mocked.impact.mockResolvedValueOnce(imp({ base: "a" }));
    await runImpact("a", null);
    mocked.impact.mockResolvedValueOnce(imp({ base: "b" }));
    await runImpact("b", null);
    expect(impact.overlays).toHaveLength(1);
    expect(impact.current?.base).toBe("b");
    expect(impact.selected?.label).toBe("b..working tree");
    clearImpact();
    expect(impact.overlays).toEqual([]);
    expect(impact.selectedId).toBeNull();
  });

  it("a kept impact stays listed; the most recent is selected; clicks switch the shown matrix", async () => {
    const work = mx("work");
    await loaded(work);
    const snapA = mx("snapA");
    mocked.impact.mockResolvedValueOnce(imp({ base: "a", head: "h", matrix: snapA }));
    await runImpact("a", "h");
    keepOverlay();
    mocked.impact.mockResolvedValueOnce(imp({ base: "b" }));
    await runImpact("b", null);
    expect(impact.overlays.map((o) => [o.label, o.pinned])).toEqual([
      ["a..h", true],
      ["b..working tree", false],
    ]);
    const [first, second] = impact.overlays;
    expect(impact.selectedId).toBe(second.id);
    expect(shown.matrix).toBe(work);
    mocked.diff.mockResolvedValue({ base: "a", head: null, files: [], truncated: false });
    selectOverlay(first.id);
    expect(impact.current?.base).toBe("a");
    expect(shown.matrix).toBe(snapA);
    expect(git.diff).toMatchObject({ base: "a", head: "h" });
    // Clear drops the selection and the unpinned overlay; the kept one stays
    clearImpact();
    expect(impact.overlays.map((o) => o.id)).toEqual([first.id]);
    expect(impact.current).toBeNull();
    expect(shown.matrix).toBe(work);
  });

  it("removing the shown overlay shows the most recent one left; removing the last shows none", async () => {
    await loaded();
    mocked.impact.mockResolvedValueOnce(imp({ base: "a" }));
    await runImpact("a", null);
    keepOverlay();
    mocked.impact.mockResolvedValueOnce(imp({ base: "b" }));
    await runImpact("b", null);
    keepOverlay();
    mocked.impact.mockResolvedValueOnce(imp({ base: "c" }));
    await runImpact("c", null);
    const [a, b, c] = impact.overlays.map((o) => o.id);
    selectOverlay(a);
    removeOverlay(b);
    expect(impact.selectedId).toBe(a);
    removeOverlay(a);
    expect(impact.selectedId).toBe(c);
    removeOverlay(c);
    expect(impact.overlays).toEqual([]);
    expect(impact.current).toBeNull();
    expect(git.diff).toBeNull();
  });

  it("selecting a commit restores its history selection", async () => {
    const commits = [
      { id: "c2", short: "c2", parents: ["c1"], base: "c1", subject: "s", author: "a", date: "", refs: [] },
      { id: "c1", short: "c1", parents: [], base: "empty", subject: "s", author: "a", date: "", refs: [] },
    ];
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: false, commits });
    mocked.diff.mockResolvedValue({ base: "a", head: null, files: [], truncated: false });
    mocked.impact.mockResolvedValue(imp());
    await load("/p");
    pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    await flush();
    keepOverlay();
    runTypedImpact("x", null);
    await flush();
    expect(git.historySel).toBeNull();
    selectOverlay(impact.overlays[0].id);
    expect(git.historySel).toEqual({ kind: "range", newer: 0, older: 0, anchor: 0 });
  });

  it("a rules reload re-runs the shown overlay; another one is re-run when selected", async () => {
    await loaded(withCells(7, ["m1", 1]));
    mocked.impact.mockResolvedValueOnce(imp({ base: "a" }));
    await runImpact("a", null);
    keepOverlay();
    mocked.impact.mockResolvedValueOnce(imp({ base: "b" }));
    await runImpact("b", null);
    mocked.reclassify.mockResolvedValue(withCells(8, ["m1", 1]));
    mocked.impact.mockResolvedValue(imp({ base: "new" }));
    mocked.impact.mockClear();
    await reloadRules();
    await flush();
    expect(mocked.impact).toHaveBeenCalledTimes(1);
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "b", null, expect.any(Function));
    expect(impact.overlays.map((o) => o.stale)).toEqual([true, false]);
    selectOverlay(impact.overlays[0].id);
    await flush();
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "a", null, expect.any(Function));
    expect(impact.overlays.map((o) => o.stale)).toEqual([false, false]);
    expect(impact.overlays).toHaveLength(2);
  });
});

describe("impact overlay on its own", () => {
  it("abandon drops the running answer but keeps the impact shown and its range", async () => {
    const o = new ImpactOverlays();
    const first = imp({ base: "first" });
    mocked.impact.mockResolvedValueOnce(first);
    await o.run("/p", { base: "a", head: null }, (r) => o.add({ base: "a", head: null }, r));
    expect(o.current).toBe(first);
    let resolve: (i: ReturnType<typeof imp>) => void = () => {};
    mocked.impact.mockReturnValueOnce(new Promise((r) => (resolve = r)));
    const applied: unknown[] = [];
    const stale = o.run("/p", { base: "b", head: "h" }, (r) => applied.push(r));
    expect(o.busy).toBe(true);
    o.abandon();
    expect(o.busy).toBe(false);
    expect(o.current).toBe(first);
    expect(o.range).toEqual({ base: "b", head: "h" });
    resolve(imp());
    await stale;
    expect(applied).toEqual([]);
    expect(o.busy).toBe(false);
  });

  it("clear drops the impact, its range, its error and a running request", async () => {
    const o = new ImpactOverlays();
    mocked.impact.mockRejectedValueOnce(new Error("bad"));
    await o.run("/p", { base: "a", head: null }, (r) => o.add({ base: "a", head: null }, r));
    expect(o.error).toContain("bad");
    let resolve: (i: ReturnType<typeof imp>) => void = () => {};
    mocked.impact.mockReturnValueOnce(new Promise((r) => (resolve = r)));
    const run = o.run("/p", { base: "a", head: null }, (r) => o.add({ base: "a", head: null }, r));
    o.clear();
    resolve(imp());
    await run;
    expect([o.current, o.range, o.error, o.busy, o.progress]).toEqual([null, null, "", false, null]);
  });
});
