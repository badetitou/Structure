import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "../api";
import type { Backend, WireImpact, WireMatrix } from "../api";
import { viewerBackend } from "./backend";
import { cellSymbolsClient } from "./protocol";

const wireMatrix: WireMatrix = {
  project: "p",
  language: "java",
  layers: [{ id: "business", label: "business", order: 4 }],
  domains: ["account"],
  cells: [{ id: "business:account", layer: "business", domain: "account", size: 2 }],
  edges: [],
  stats: { files: 1, symbols: 2, edges: 0, elapsed_ms: 1 },
};
const wireImpact: WireImpact = {
  base: "a",
  head: "b",
  changed_files: [],
  changed_symbols: [],
  changed_cells: ["business:account"],
  impacted_cells: [],
  impacted_symbols: [],
  matrix: wireMatrix,
};
const page = { total: 0, offset: 0, symbols: [] };

describe("viewerBackend", () => {
  it("analyze and impact share the expanded matrix of the impact", async () => {
    const b = viewerBackend({ impact: wireImpact, cellSymbols: async () => page });
    const m = await b.analyze(["x"], () => {});
    const imp = await b.impact("x", null, null);
    expect(m.cells).toHaveLength(1);
    expect(m.httpEdges).toEqual([]);
    expect(imp.matrix).toBe(m);
    expect(imp.changed_cells).toEqual(["business:account"]);
  });

  it("forwards cell symbol requests without paths, head or generation", async () => {
    const cellSymbols = vi.fn(async () => page);
    const b = viewerBackend({ impact: wireImpact, cellSymbols });
    await expect(b.cellSymbols(["x"], null, 3, ["c"], 5, 10)).resolves.toBe(page);
    expect(cellSymbols).toHaveBeenCalledWith(["c"], 5, 10);
  });

  it("declares no history, diff, folder picker or rules reload, and leaves those methods out", () => {
    const b = viewerBackend({ impact: wireImpact, cellSymbols: async () => page });
    expect(b.capabilities).toEqual({
      history: false,
      diff: false,
      pickFolder: false,
      reclassify: false,
      highlightSources: ["git-diff"],
    });
    for (const m of ["history", "diff", "pickFolder", "reclassify", "rulesStamp"] as const) expect(b[m]).toBeUndefined();
  });

  it("refuses an impact without matrix", () => {
    const { matrix: _m, ...bare } = wireImpact;
    expect(() => viewerBackend({ impact: bare, cellSymbols: async () => page })).toThrow(/no matrix/);
  });
});

describe("cellSymbolsClient", () => {
  it("posts a request and resolves with the matching result", async () => {
    const post = vi.fn();
    const c = cellSymbolsClient({ postMessage: post }, "*");
    const p = c.request(["c"], 0, 5);
    expect(post).toHaveBeenCalledWith({ type: "structure:cell-symbols", id: 1, cells: ["c"], offset: 0, limit: 5 }, "*");
    c.onMessage({ type: "structure:cell-symbols:result", id: 1, result: page });
    await expect(p).resolves.toBe(page);
  });
});

describe("api backend seam", () => {
  afterEach(() => api.setBackend(api.defaultBackend));

  it("delegates the exported functions to the current backend", async () => {
    const fake: Backend = {
      capabilities: { history: true, diff: true, pickFolder: true, reclassify: true, highlightSources: [] },
      pickFolder: vi.fn(async () => "f"),
      analyze: vi.fn(async () => ({}) as never),
      reclassify: vi.fn(async () => ({}) as never),
      rulesStamp: vi.fn(async () => null),
      impact: vi.fn(async () => ({}) as never),
      cellSymbols: vi.fn(async () => page),
      history: vi.fn(async () => ({ dirty: false, commits: [] })),
      diff: vi.fn(async () => ({ base: "a", head: null, files: [], truncated: false })),
    };
    api.setBackend(fake);
    expect(await api.pickFolder()).toBe("f");
    await api.cellSymbols(["p"], null, 1, ["c"], 0, 2);
    expect(fake.cellSymbols).toHaveBeenCalledWith(["p"], null, 1, ["c"], 0, 2);
    await api.history("p", 3);
    expect(fake.history).toHaveBeenCalledWith("p", 3);
    expect(api.capabilities()).toBe(fake.capabilities);
  });

  it("rejects a call to a method the backend leaves out", async () => {
    api.setBackend(viewerBackend({ impact: wireImpact, cellSymbols: async () => page }));
    expect(api.capabilities().history).toBe(false);
    await expect(api.history("p")).rejects.toThrow(/does not support history/);
    await expect(api.reclassify(["p"])).rejects.toThrow(/does not support reclassify/);
  });

  it("declares what the samples support: everything but reloading the rules", () => {
    const b = api.defaultBackend;
    expect(b.capabilities).toEqual({
      history: true,
      diff: true,
      pickFolder: true,
      reclassify: false,
      highlightSources: ["git-diff"],
    });
    expect(b.reclassify).toBeUndefined();
    expect(b.rulesStamp).toBeUndefined();
    for (const m of ["history", "diff", "pickFolder"] as const) expect(b[m]).toBeTypeOf("function");
  });

  it("uses the sample files by default (outside Tauri)", async () => {
    const fetchMock = vi.fn(async () => ({ ok: true, json: async () => ({ dirty: false, commits: [] }) }));
    vi.stubGlobal("fetch", fetchMock);
    try {
      await expect(api.history("p")).resolves.toEqual({ dirty: false, commits: [] });
      expect(fetchMock).toHaveBeenCalledWith("/samples/history.json");
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
