import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Impact, Matrix } from "./api";
import type { ViewCell } from "./domaintree";

vi.mock("./api", () => ({
  analyze: vi.fn(),
  reclassify: vi.fn(),
  rulesStamp: vi.fn(),
  impact: vi.fn(),
  cellSymbols: vi.fn(),
  history: vi.fn(),
  diff: vi.fn(),
}));

import * as api from "./api";
import { AppState } from "./state.svelte";

const mx = (project: string, warnings?: string[]): Matrix => ({
  project,
  language: "java",
  layers: [],
  domains: [],
  cells: [],
  edges: [],
  httpEdges: [],
  rpcEdges: [],
  stats: { files: 1, symbols: 1, edges: 0, elapsed_ms: 1 },
  generation: 7,
  warnings,
});
const imp = (over: Partial<Impact> = {}): Impact => ({
  base: "a",
  head: null,
  changed_files: [],
  changed_symbols: [],
  changed_cells: [],
  impacted_cells: [],
  impacted_symbols: [],
  ...over,
});
const cell = { id: "c1", layer: "service", label: "orders", size: 2, members: ["m1"] } as unknown as ViewCell;

const mocked = vi.mocked(api);
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  vi.resetAllMocks();
});

async function loaded(work = mx("work")) {
  mocked.analyze.mockResolvedValue(work);
  mocked.history.mockResolvedValue({ dirty: false, commits: [] });
  const s = new AppState();
  await s.load("/p");
  return s;
}

const twoProjects = [
  { name: "front", root: "/front", language: "typescript" as const },
  { name: "back", root: "/back", language: "java" as const },
];

describe("AppState", () => {
  it("loads the analysis, reports progress and then the history", async () => {
    const work = mx("p");
    mocked.analyze.mockImplementation(async (_p, onProgress) => {
      onProgress({ step: "parsing", pct: 40 });
      return work;
    });
    mocked.history.mockResolvedValue({ dirty: true, commits: [] });
    const s = new AppState();
    const done = s.load("/p");
    expect(s.phase).toBe("loading");
    await done;
    expect(s.phase).toBe("ready");
    expect(s.progress.pct).toBe(40);
    expect(s.matrix).toBe(work);
    expect(s.history?.dirty).toBe(true);
  });

  it("keeps working without history", async () => {
    mocked.analyze.mockResolvedValue(mx("p"));
    mocked.history.mockRejectedValue(new Error("not a repo"));
    const s = new AppState();
    await s.load("/p");
    expect(s.phase).toBe("ready");
    expect(s.history).toBeNull();
  });

  it("analyses several folders together without any git call", async () => {
    mocked.analyze.mockResolvedValue({ ...mx("front"), projects: twoProjects });
    const s = new AppState();
    await s.load(["/front", "/back"]);
    expect(mocked.analyze.mock.calls[0][0]).toEqual(["/front", "/back"]);
    expect(s.multi).toBe(true);
    expect(s.phase).toBe("ready");
    expect(mocked.history).not.toHaveBeenCalled();
    await s.runImpact(null, null);
    await s.loadDiff(null, null);
    expect(mocked.impact).not.toHaveBeenCalled();
    expect(mocked.diff).not.toHaveBeenCalled();
    s.selectCell(cell);
    await flush();
    expect(mocked.cellSymbols.mock.calls[0][0]).toEqual(["/front", "/back"]);
  });

  it("is not multi when two paths resolve to one project", async () => {
    mocked.analyze.mockResolvedValue(mx("p"));
    mocked.history.mockResolvedValue({ dirty: false, commits: [] });
    const s = new AppState();
    await s.load(["/p", "/p/"]);
    expect(s.multi).toBe(false);
    expect(mocked.history).toHaveBeenCalledTimes(1);
    expect(s.history).not.toBeNull();
  });

  it("goes single -> multi -> single with clean git state each time", async () => {
    const hist = { dirty: false, commits: [] };
    mocked.history.mockResolvedValue(hist);
    mocked.impact.mockResolvedValue(imp());
    mocked.diff.mockResolvedValue({ base: "a", head: null, files: [], truncated: false });
    const s = new AppState();
    mocked.analyze.mockResolvedValue(mx("p"));
    await s.load("/p");
    await s.runImpact(null, null);
    await s.loadDiff(null, null);
    expect(s.history).toBe(hist);
    expect(s.impact).not.toBeNull();
    expect(s.diff).not.toBeNull();

    mocked.history.mockClear();
    mocked.analyze.mockResolvedValue({ ...mx("front"), projects: twoProjects });
    await s.load(["/front", "/back"]);
    expect(s.multi).toBe(true);
    for (const x of [s.history, s.impact, s.diff]) expect(x).toBeNull();
    expect(s.impactBusy).toBe(false);
    expect(s.impactError).toBe("");
    expect(mocked.history).not.toHaveBeenCalled();

    mocked.analyze.mockResolvedValue(mx("p"));
    await s.load("/p");
    expect(s.multi).toBe(false);
    expect(s.history).toBe(hist);
    for (const x of [s.impact, s.diff]) expect(x).toBeNull();
    expect(s.impactBusy).toBe(false);
    expect(s.impactError).toBe("");
  });

  it("rejects when the analysis fails", async () => {
    mocked.analyze.mockRejectedValue(new Error("boom"));
    const s = new AppState();
    await expect(s.load("/p")).rejects.toThrow("boom");
  });

  it("shows the snapshot matrix of an impact, and its warnings", async () => {
    const work = mx("work", ["w"]);
    const snap = mx("snap");
    const s = await loaded(work);
    expect(s.warnings).toEqual(["w"]);
    mocked.impact.mockResolvedValue(imp({ head: "abc", matrix: snap, warnings: ["x"] }));
    await s.runImpact("old", "abc");
    expect(s.matrix).toBe(snap);
    expect(s.warnings).toEqual(["x"]);
    s.clearImpact();
    expect(s.matrix).toBe(work);
    expect(s.impact).toBeNull();
  });

  it("drops the selection only when the shown matrix changes", async () => {
    const s = await loaded();
    mocked.cellSymbols.mockResolvedValue({ total: 2, offset: 0, symbols: [] });
    s.selectCell(cell);
    mocked.impact.mockResolvedValue(imp());
    await s.runImpact(null, null);
    expect(s.selected).toBe(cell);
    mocked.impact.mockResolvedValue(imp({ head: "h", matrix: mx("snap") }));
    await s.runImpact("a", "h");
    expect(s.selected).toBeNull();
  });

  it("ignores an impact answer superseded by Clear", async () => {
    const s = await loaded();
    let resolve: (i: Impact) => void = () => {};
    mocked.impact.mockReturnValue(new Promise((r) => (resolve = r)));
    const run = s.runImpact(null, null);
    expect(s.impactBusy).toBe(true);
    s.clearImpact();
    resolve(imp());
    await run;
    expect(s.impact).toBeNull();
    expect(s.impactBusy).toBe(false);
  });

  it("history selection runs the matching impact; a typed diff clears the selection", async () => {
    const commits = [
      { id: "c2", short: "c2", parents: ["c1"], base: "c1", subject: "s", author: "a", date: "", refs: [] },
      { id: "c1", short: "c1", parents: [], base: "empty", subject: "s", author: "a", date: "", refs: [] },
    ];
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: false, commits });
    mocked.impact.mockResolvedValue(imp());
    const s = new AppState();
    await s.load("/p");
    s.pickHistory({ kind: "range", newer: 0, older: 1, anchor: 0 });
    expect(mocked.impact).toHaveBeenCalledWith("/p", "empty", "c2", expect.any(Function));
    expect(s.historySel?.kind).toBe("range");
    await flush();
    s.runTypedImpact("x", null);
    expect(s.historySel).toBeNull();
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "x", null, expect.any(Function));
  });

  it("pages the symbols of the selected cell and reports errors", async () => {
    const s = await loaded();
    const sym = (name: string) => ({ id: name, name, file: "f" });
    mocked.cellSymbols.mockResolvedValueOnce({ total: 2, offset: 0, symbols: [sym("a")] });
    s.selectCell(cell);
    expect(s.symbols?.loading).toBe(true);
    await flush();
    expect(mocked.cellSymbols).toHaveBeenCalledWith(["/p"], null, 7, ["m1"], 0, expect.any(Number));
    expect(s.symbols?.items).toHaveLength(1);
    mocked.cellSymbols.mockResolvedValueOnce({ total: 2, offset: 1, symbols: [sym("b")] });
    await s.loadSymbols(true);
    expect(s.symbols?.items).toHaveLength(2);
    mocked.cellSymbols.mockRejectedValueOnce(new Error("nope"));
    await s.loadSymbols(false);
    expect(s.symbols?.error).toContain("nope");
    s.selectCell(null);
    expect(s.symbols).toBeNull();
  });

  it("toggles dense mode and persists it, whatever the stored value was", () => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    });
    try {
      const s = new AppState();
      expect(s.mode).toBe("normal");
      s.setMode("dense");
      expect(s.mode).toBe("dense");
      expect(store.get("structure.matrix.mode")).toBe("dense");
      s.setMode("flame");
      expect(s.mode).toBe("flame");
      expect(store.get("structure.matrix.mode")).toBe("flame");
      s.setMode("normal");
      expect(store.get("structure.matrix.mode")).toBe("normal");
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("ignores a superseded load, whether it resolves or rejects", async () => {
    mocked.history.mockResolvedValue({ dirty: false, commits: [] });
    const s = new AppState();
    let resolveA: (m: Matrix) => void = () => {};
    mocked.analyze.mockReturnValueOnce(new Promise<Matrix>((r) => (resolveA = r)));
    const loadA = s.load("/a");
    mocked.analyze.mockResolvedValueOnce(mx("B"));
    await s.load("/b");
    resolveA(mx("A"));
    await expect(loadA).resolves.toBeUndefined();
    expect(s.matrix?.project).toBe("B");
    expect(s.project).toBe("/b");

    let rejectC: (e: unknown) => void = () => {};
    mocked.analyze.mockReturnValueOnce(new Promise<Matrix>((_, r) => (rejectC = r)));
    const loadC = s.load("/c");
    s.reset();
    rejectC(new Error("late"));
    await expect(loadC).resolves.toBeUndefined();
  });

  it("ignores new diffs while an impact runs and reports its progress", async () => {
    const commits = [{ id: "c1", short: "c1", parents: [], base: "e", subject: "s", author: "a", date: "", refs: [] }];
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits });
    const s = new AppState();
    await s.load("/p");
    mocked.impact.mockClear();
    let resolve: (i: Impact) => void = () => {};
    let report: (p: { step: string; pct: number }) => void = () => {};
    mocked.impact.mockImplementationOnce((_p, _b, _h, onProgress) => {
      report = onProgress ?? report;
      return new Promise((r) => (resolve = r));
    });
    s.pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    expect(s.impactBusy).toBe(true);
    expect(s.impactProgress?.pct).toBe(0);
    report({ step: "parsing", pct: 40 });
    expect(s.impactProgress).toEqual({ step: "parsing", pct: 40 });

    s.pickHistory({ kind: "worktree" });
    s.runTypedImpact("x", null);
    await s.runImpact("c", "d");
    expect(mocked.impact).toHaveBeenCalledTimes(1);
    expect(s.historySel).toEqual({ kind: "range", newer: 0, older: 0, anchor: 0 });

    resolve(imp());
    await flush();
    expect(s.impactBusy).toBe(false);
    expect(s.impactProgress).toBeNull();
  });

  it("an impact failure is reported and cleared by Clear", async () => {
    const s = await loaded();
    mocked.impact.mockRejectedValueOnce(new Error("bad ref"));
    await s.runImpact("c", "d");
    expect(s.impactBusy).toBe(false);
    expect(s.impactProgress).toBeNull();
    expect(s.impactError).toContain("bad ref");
    s.clearImpact();
    expect(s.impactError).toBe("");
  });

  it("loads the diff of the history selection, ignores superseded answers and clears it", async () => {
    const c = (id: string, base: string) => ({ id, short: id, parents: [], base, subject: "s", author: "a", date: "", refs: [] });
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: false, commits: [c("c2", "c1"), c("c1", "e")] });
    mocked.impact.mockResolvedValue(imp());
    const late = { base: "c1", head: "c2", files: [], truncated: false };
    const fresh = { base: "e", head: "c1", files: [], truncated: false };
    let release: (v: typeof late) => void = () => {};
    mocked.diff.mockReturnValueOnce(new Promise((r) => (release = r))).mockResolvedValueOnce(fresh);
    const s = new AppState();
    await s.load("/p");
    expect(s.diff).toBeNull();
    s.pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    expect(mocked.diff).toHaveBeenLastCalledWith("/p", "c1", "c2");
    expect(s.diff).toEqual({ base: "c1", head: "c2", loading: true });
    await flush();
    s.runTypedImpact("e", "c1");
    await flush();
    release(late);
    await flush();
    expect(s.diff).toEqual({ base: "e", head: "c1", loading: false, data: fresh });
    s.clearImpact();
    expect(s.diff).toBeNull();
  });

  it("ignores a diff answer arriving after Clear, and loads the working tree diff", async () => {
    const c = (id: string, base: string) => ({ id, short: id, parents: [], base, subject: "s", author: "a", date: "", refs: [] });
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits: [c("c2", "c1")] });
    mocked.impact.mockResolvedValue(imp());
    let release: (v: { base: string; head: string; files: []; truncated: boolean }) => void = () => {};
    mocked.diff.mockReturnValueOnce(new Promise((r) => (release = r)));
    const s = new AppState();
    await s.load("/p");
    s.pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    s.clearImpact();
    release({ base: "c1", head: "c2", files: [], truncated: false });
    await flush();
    expect(s.diff).toBeNull();
    mocked.diff.mockResolvedValue({ base: "HEAD", head: null, files: [], truncated: false });
    s.pickHistory({ kind: "worktree" });
    expect(mocked.diff).toHaveBeenLastCalledWith("/p", null, null);
  });

  it("reports a diff failure", async () => {
    const s = await loaded();
    mocked.diff.mockRejectedValue(new Error("boom"));
    await s.loadDiff(null, null);
    expect(s.diff).toEqual({ base: null, head: null, loading: false, error: "Error: boom" });
  });

  it("selects the working tree and single commits", async () => {
    const c = (id: string, base: string) => ({ id, short: id, parents: [], base, subject: "s", author: "a", date: "", refs: [] });
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits: [c("c2", "c1"), c("c1", "e")] });
    mocked.impact.mockResolvedValue(imp());
    const s = new AppState();
    await s.load("/p");
    s.pickHistory({ kind: "worktree" });
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", null, null, expect.any(Function));
    await flush();
    s.pickHistory({ kind: "range", newer: 1, older: 1, anchor: 1 });
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "e", "c1", expect.any(Function));
  });

  it("lists snapshot symbols against the snapshot head", async () => {
    const s = await loaded();
    mocked.impact.mockResolvedValue(imp({ head: "h1", matrix: mx("snap") }));
    await s.runImpact("a", "h1");
    mocked.cellSymbols.mockResolvedValue({ total: 0, offset: 0, symbols: [] });
    s.selectCell(cell);
    await flush();
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], "h1", 7, ["m1"], 0, expect.any(Number));
  });
});

describe("AppState rules reload", () => {
  const withCells = (generation: number, ...ids: [string, number][]): Matrix => ({
    ...mx("p"),
    generation,
    cells: ids.map(([id, size]) => ({ id, layer: "service", domain: id, size })),
  });
  const page = { total: 0, offset: 0, symbols: [] };

  it("keeps the selection whose cell still exists and lists it against the new matrix", async () => {
    const s = await loaded(withCells(7, ["m1", 2]));
    mocked.cellSymbols.mockResolvedValue(page);
    s.selectCell(cell);
    const next = withCells(8, ["m1", 5], ["m2", 1]);
    mocked.reclassify.mockResolvedValue(next);
    await s.reloadRules();
    expect(mocked.reclassify).toHaveBeenCalledWith(["/p"]);
    expect(s.matrix).toBe(next);
    expect(s.selected?.id).toBe("c1");
    expect(s.selected?.size).toBe(5);
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], null, 8, ["m1"], 0, expect.any(Number));
    expect(s.rulesBusy).toBe(false);
  });

  it("drops the selection whose cell is gone", async () => {
    const s = await loaded(withCells(7, ["m1", 2]));
    mocked.cellSymbols.mockResolvedValue(page);
    s.selectCell(cell);
    mocked.reclassify.mockResolvedValue(withCells(8, ["m2", 2]));
    await s.reloadRules();
    expect(s.selected).toBeNull();
  });

  it("keeps the matrix and the selection when the rules are invalid", async () => {
    const work = withCells(7, ["m1", 2]);
    const s = await loaded(work);
    mocked.cellSymbols.mockResolvedValue(page);
    s.selectCell(cell);
    mocked.reclassify.mockRejectedValue(new Error("invalid .structure/rules.toml"));
    await s.reloadRules();
    expect(s.matrix).toBe(work);
    expect(s.selected).toBe(cell);
    expect(s.phase).toBe("ready");
    expect(s.rulesError).toBe("Rules reload failed: Error: invalid .structure/rules.toml");
    expect(s.rulesBusy).toBe(false);
    // The next successful reload clears it.
    mocked.reclassify.mockResolvedValue(withCells(8, ["m1", 2]));
    await s.reloadRules();
    expect(s.rulesError).toBe("");
  });

  it("runs the impact shown again, keeping a selection that exists in the new snapshot", async () => {
    const s = await loaded(withCells(7, ["m1", 2]));
    mocked.cellSymbols.mockResolvedValue(page);
    mocked.impact.mockResolvedValue(imp({ head: "h", matrix: withCells(9, ["m1", 1]) }));
    await s.runImpact("a", "h");
    s.selectCell(cell);
    const snap = withCells(11, ["m1", 3]);
    mocked.impact.mockResolvedValue(imp({ head: "h", matrix: snap }));
    mocked.reclassify.mockResolvedValue(withCells(10, ["m1", 2]));
    await s.reloadRules();
    await flush();
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "a", "h", expect.any(Function));
    expect(s.matrix).toBe(snap);
    expect(s.selected?.size).toBe(3);
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], "h", 11, ["m1"], 0, expect.any(Number));
  });

  it("does not run a cleared impact again", async () => {
    const s = await loaded();
    mocked.impact.mockResolvedValue(imp());
    await s.runImpact(null, null);
    s.clearImpact();
    mocked.impact.mockClear();
    mocked.reclassify.mockResolvedValue(mx("p"));
    await s.reloadRules();
    await flush();
    expect(mocked.impact).not.toHaveBeenCalled();
  });

  it("reloads when the rules stamp changes, not before", async () => {
    mocked.rulesStamp.mockResolvedValue("s1");
    const s = await loaded();
    await flush();
    mocked.reclassify.mockResolvedValue(mx("p"));
    await s.checkRules();
    expect(mocked.reclassify).not.toHaveBeenCalled();
    mocked.rulesStamp.mockResolvedValue("s2");
    await s.checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
    await s.checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
    // Nothing to watch (samples, viewer) or a failing stamp: no reload.
    mocked.rulesStamp.mockResolvedValue(null);
    await s.checkRules();
    mocked.rulesStamp.mockRejectedValue(new Error("gone"));
    await s.checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
  });

  it("does not reload again on the next poll after a manual reload", async () => {
    mocked.rulesStamp.mockResolvedValue("s1");
    const s = await loaded();
    await flush();
    mocked.reclassify.mockResolvedValue(mx("p"));
    await s.checkRules();
    mocked.rulesStamp.mockResolvedValue("s2");
    await s.reloadRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
    await s.checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
  });

  it("follows the view's cell of the same id, or drops a selection that left the view", async () => {
    const s = await loaded();
    mocked.cellSymbols.mockResolvedValue(page);
    s.selectCell(cell);
    s.syncSelection([cell]);
    expect(s.selected).toBe(cell);
    const wider = { ...cell, size: 3, members: ["m1", "m2"] } as ViewCell;
    s.syncSelection([wider]);
    expect(s.selected).toBe(wider);
    s.syncSelection([]);
    expect(s.selected).toBeNull();
  });
});
