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

import { ImpactOverlay, impact } from "./impact.svelte";
import { selection } from "./selection.svelte";
import { git } from "./git.svelte";
import { shown } from "./shown";
import { load, runImpact, clearImpact, pickHistory, runTypedImpact } from "./session";
import { cell, flush, imp, loaded, mocked, mx, setup } from "./testing";

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

describe("impact overlay on its own", () => {
  it("abandon drops the running answer but keeps the impact shown and its range", async () => {
    const o = new ImpactOverlay();
    const first = imp({ base: "first" });
    mocked.impact.mockResolvedValueOnce(first);
    await o.run("/p", { base: "a", head: null }, (r) => (o.current = r));
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
    const o = new ImpactOverlay();
    mocked.impact.mockRejectedValueOnce(new Error("bad"));
    await o.run("/p", { base: "a", head: null }, (r) => (o.current = r));
    expect(o.error).toContain("bad");
    let resolve: (i: ReturnType<typeof imp>) => void = () => {};
    mocked.impact.mockReturnValueOnce(new Promise((r) => (resolve = r)));
    const run = o.run("/p", { base: "a", head: null }, (r) => (o.current = r));
    o.clear();
    resolve(imp());
    await run;
    expect([o.current, o.range, o.error, o.busy, o.progress]).toEqual([null, null, "", false, null]);
  });
});
