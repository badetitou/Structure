import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Impact, Matrix } from "./api";
import type { ViewCell } from "./domaintree";

vi.mock("./api", () => ({
  analyze: vi.fn(),
  impact: vi.fn(),
  cellSymbols: vi.fn(),
  history: vi.fn(),
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
    expect(mocked.cellSymbols).toHaveBeenCalledWith("/p", null, 7, ["m1"], 0, expect.any(Number));
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
      s.toggleMode();
      expect(s.mode).toBe("dense");
      expect(store.get("structure.matrix.mode")).toBe("dense");
      s.toggleMode();
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
    expect(s.impactError).toContain("bad ref");
    s.clearImpact();
    expect(s.impactError).toBe("");
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
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith("/p", "h1", 7, ["m1"], 0, expect.any(Number));
  });
});
