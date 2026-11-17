import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Matrix } from "../api";

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

import { analysis } from "./analysis.svelte";
import { impact } from "./impact.svelte";
import { selection } from "./selection.svelte";
import { git } from "./git.svelte";
import { shown } from "./shown";
import { load, reset, runImpact, clearImpact, reloadRules, checkRules, watchRules } from "./session";
import { allCaps, cell, flush, imp, loaded, mocked, mx, page, setup, twoProjects, viewerCaps, withCells } from "./testing";

beforeEach(setup);

describe("analysis and load", () => {
  it("loads the analysis, reports progress and then the history", async () => {
    const work = mx("p");
    mocked.analyze.mockImplementation(async (_p, onProgress) => {
      onProgress({ step: "parsing", pct: 40 });
      return work;
    });
    mocked.history.mockResolvedValue({ dirty: true, commits: [] });
    const done = load("/p");
    expect(analysis.phase).toBe("loading");
    await done;
    expect(analysis.phase).toBe("ready");
    expect(analysis.progress.pct).toBe(40);
    expect(shown.matrix).toBe(work);
    expect(git.history?.dirty).toBe(true);
  });

  it("keeps working without history", async () => {
    mocked.analyze.mockResolvedValue(mx("p"));
    mocked.history.mockRejectedValue(new Error("not a repo"));
    await load("/p");
    expect(analysis.phase).toBe("ready");
    expect(git.history).toBeNull();
  });

  it("analyses several folders together without any git call", async () => {
    mocked.analyze.mockResolvedValue({ ...mx("front"), projects: twoProjects });
    await load(["/front", "/back"]);
    expect(mocked.analyze.mock.calls[0][0]).toEqual(["/front", "/back"]);
    expect(analysis.multi).toBe(true);
    expect(analysis.phase).toBe("ready");
    expect(mocked.history).not.toHaveBeenCalled();
    await runImpact(null, null);
    await git.loadDiff(null, null);
    expect(mocked.impact).not.toHaveBeenCalled();
    expect(mocked.diff).not.toHaveBeenCalled();
    selection.select(cell);
    await flush();
    expect(mocked.cellSymbols.mock.calls[0][0]).toEqual(["/front", "/back"]);
  });

  it("is not multi when two paths resolve to one project", async () => {
    mocked.analyze.mockResolvedValue(mx("p"));
    mocked.history.mockResolvedValue({ dirty: false, commits: [] });
    await load(["/p", "/p/"]);
    expect(analysis.multi).toBe(false);
    expect(mocked.history).toHaveBeenCalledTimes(1);
    expect(git.history).not.toBeNull();
  });

  it("is not multi when one root holds several languages", async () => {
    const projects = [
      { name: "app (java)", root: "/app", language: "java" as const },
      { name: "app (typescript)", root: "/app", language: "typescript" as const },
    ];
    mocked.analyze.mockResolvedValue({ ...mx("app (java)"), projects });
    mocked.history.mockResolvedValue({ dirty: false, commits: [] });
    await load("/app");
    expect(analysis.multi).toBe(false);
    expect(mocked.history).toHaveBeenCalledTimes(1);
    expect(git.history).not.toBeNull();
  });

  it("goes single -> multi -> single with clean git state each time", async () => {
    const hist = { dirty: false, commits: [] };
    mocked.history.mockResolvedValue(hist);
    mocked.impact.mockResolvedValue(imp());
    mocked.diff.mockResolvedValue({ base: "a", head: null, files: [], truncated: false });
    mocked.analyze.mockResolvedValue(mx("p"));
    await load("/p");
    await runImpact(null, null);
    await git.loadDiff(null, null);
    expect(git.history).toBe(hist);
    expect(impact.current).not.toBeNull();
    expect(git.diff).not.toBeNull();

    mocked.history.mockClear();
    mocked.analyze.mockResolvedValue({ ...mx("front"), projects: twoProjects });
    await load(["/front", "/back"]);
    expect(analysis.multi).toBe(true);
    for (const x of [git.history, impact.current, git.diff]) expect(x).toBeNull();
    expect(impact.busy).toBe(false);
    expect(impact.error).toBe("");
    expect(mocked.history).not.toHaveBeenCalled();

    mocked.analyze.mockResolvedValue(mx("p"));
    await load("/p");
    expect(analysis.multi).toBe(false);
    expect(git.history).toBe(hist);
    for (const x of [impact.current, git.diff]) expect(x).toBeNull();
    expect(impact.busy).toBe(false);
    expect(impact.error).toBe("");
  });

  it("rejects when the analysis fails", async () => {
    mocked.analyze.mockRejectedValue(new Error("boom"));
    await expect(load("/p")).rejects.toThrow("boom");
  });

  it("ignores a superseded load, whether it resolves or rejects", async () => {
    mocked.history.mockResolvedValue({ dirty: false, commits: [] });
    let resolveA: (m: Matrix) => void = () => {};
    mocked.analyze.mockReturnValueOnce(new Promise<Matrix>((r) => (resolveA = r)));
    const loadA = load("/a");
    mocked.analyze.mockResolvedValueOnce(mx("B"));
    await load("/b");
    resolveA(mx("A"));
    await expect(loadA).resolves.toBeUndefined();
    expect(shown.matrix?.project).toBe("B");
    expect(analysis.project).toBe("/b");

    let rejectC: (e: unknown) => void = () => {};
    mocked.analyze.mockReturnValueOnce(new Promise<Matrix>((_, r) => (rejectC = r)));
    const loadC = load("/c");
    reset();
    rejectC(new Error("late"));
    await expect(loadC).resolves.toBeUndefined();
  });
});

describe("rules reload", () => {
  it("keeps the selection whose cell still exists and lists it against the new matrix", async () => {
    await loaded(withCells(7, ["m1", 2]));
    mocked.cellSymbols.mockResolvedValue(page);
    selection.select(cell);
    const next = withCells(8, ["m1", 5], ["m2", 1]);
    mocked.reclassify.mockResolvedValue(next);
    await reloadRules();
    expect(mocked.reclassify).toHaveBeenCalledWith(["/p"]);
    expect(shown.matrix).toBe(next);
    expect(selection.selected?.id).toBe("c1");
    expect(selection.selected?.size).toBe(5);
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], null, 8, ["m1"], 0, expect.any(Number));
    expect(analysis.rulesBusy).toBe(false);
  });

  it("drops the selection whose cell is gone", async () => {
    await loaded(withCells(7, ["m1", 2]));
    mocked.cellSymbols.mockResolvedValue(page);
    selection.select(cell);
    mocked.reclassify.mockResolvedValue(withCells(8, ["m2", 2]));
    await reloadRules();
    expect(selection.selected).toBeNull();
  });

  it("keeps the matrix and the selection when the rules are invalid", async () => {
    const work = withCells(7, ["m1", 2]);
    await loaded(work);
    mocked.cellSymbols.mockResolvedValue(page);
    selection.select(cell);
    mocked.reclassify.mockRejectedValue(new Error("invalid .structure/rules.toml"));
    await reloadRules();
    expect(shown.matrix).toBe(work);
    expect(selection.selected).toBe(cell);
    expect(analysis.phase).toBe("ready");
    expect(analysis.rulesError).toBe("Rules reload failed: Error: invalid .structure/rules.toml");
    expect(analysis.rulesBusy).toBe(false);
    // The next successful reload clears it.
    mocked.reclassify.mockResolvedValue(withCells(8, ["m1", 2]));
    await reloadRules();
    expect(analysis.rulesError).toBe("");
  });

  it("runs the impact shown again, keeping a selection that exists in the new snapshot", async () => {
    await loaded(withCells(7, ["m1", 2]));
    mocked.cellSymbols.mockResolvedValue(page);
    mocked.impact.mockResolvedValue(imp({ head: "h", matrix: withCells(9, ["m1", 1]) }));
    await runImpact("a", "h");
    selection.select(cell);
    const snap = withCells(11, ["m1", 3]);
    mocked.impact.mockResolvedValue(imp({ head: "h", matrix: snap }));
    mocked.reclassify.mockResolvedValue(withCells(10, ["m1", 2]));
    await reloadRules();
    await flush();
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "a", "h", expect.any(Function));
    expect(shown.matrix).toBe(snap);
    expect(selection.selected?.size).toBe(3);
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], "h", 11, ["m1"], 0, expect.any(Number));
  });

  it("does not run a cleared impact again", async () => {
    await loaded();
    mocked.impact.mockResolvedValue(imp());
    await runImpact(null, null);
    clearImpact();
    mocked.impact.mockClear();
    mocked.reclassify.mockResolvedValue(mx("p"));
    await reloadRules();
    await flush();
    expect(mocked.impact).not.toHaveBeenCalled();
  });

  it("reloads when the rules stamp changes, not before", async () => {
    mocked.rulesStamp.mockResolvedValue("s1");
    await loaded();
    await flush();
    mocked.reclassify.mockResolvedValue(mx("p"));
    await checkRules();
    expect(mocked.reclassify).not.toHaveBeenCalled();
    mocked.rulesStamp.mockResolvedValue("s2");
    await checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
    await checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
    // Nothing to watch (samples, viewer) or a failing stamp: no reload.
    mocked.rulesStamp.mockResolvedValue(null);
    await checkRules();
    mocked.rulesStamp.mockRejectedValue(new Error("gone"));
    await checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
  });

  it("does not reload again on the next poll after a manual reload", async () => {
    mocked.rulesStamp.mockResolvedValue("s1");
    await loaded();
    await flush();
    mocked.reclassify.mockResolvedValue(mx("p"));
    await checkRules();
    mocked.rulesStamp.mockResolvedValue("s2");
    await reloadRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
    await checkRules();
    expect(mocked.reclassify).toHaveBeenCalledTimes(1);
  });
});

describe("capabilities", () => {
  it("offers history, diff and rules reload when the backend supports them", async () => {
    await loaded();
    expect(git.historyAvailable).toBe(true);
    expect(git.diffAvailable).toBe(true);
    expect(analysis.rulesReloadable).toBe(true);
  });

  it("does not ask an unsupporting backend for history, diff or rules", async () => {
    mocked.capabilities.mockReturnValue(viewerCaps);
    await loaded();
    expect(analysis.phase).toBe("ready");
    expect(mocked.history).not.toHaveBeenCalled();
    expect(git.historyAvailable).toBe(false);
    expect(git.diffAvailable).toBe(false);
    expect(analysis.rulesReloadable).toBe(false);
    await git.loadDiff(null, null);
    expect(mocked.diff).not.toHaveBeenCalled();
    expect(git.diff).toBeNull();
    await reloadRules();
    await checkRules();
    expect(mocked.rulesStamp).not.toHaveBeenCalled();
    expect(mocked.reclassify).not.toHaveBeenCalled();
    const setInterval = vi.spyOn(globalThis, "setInterval");
    try {
      watchRules()();
      expect(setInterval).not.toHaveBeenCalled();
    } finally {
      setInterval.mockRestore();
    }
  });

  it("offers history without diff when only the history is supported", async () => {
    mocked.capabilities.mockReturnValue({ ...allCaps, diff: false });
    await loaded();
    expect(git.historyAvailable).toBe(true);
    expect(git.diffAvailable).toBe(false);
    await git.loadDiff(null, null);
    expect(mocked.diff).not.toHaveBeenCalled();
    expect(git.diff).toBeNull();
  });
});
