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
    expect(mocked.impact).toHaveBeenCalledWith("/p", "empty", "c2");
    expect(s.historySel?.kind).toBe("range");
    await flush();
    s.runTypedImpact("x", null);
    expect(s.historySel).toBeNull();
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "x", null);
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

  it("toggles dense mode", () => {
    const s = new AppState();
    const before = s.mode;
    s.toggleMode();
    expect(s.mode).not.toBe(before);
  });
});
