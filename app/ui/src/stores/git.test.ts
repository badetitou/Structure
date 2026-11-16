import { beforeEach, describe, expect, it, vi } from "vitest";

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

import { GitStore, git } from "./git.svelte";
import { load, clearImpact, pickHistory, runTypedImpact } from "./session";
import { flush, imp, loaded, mocked, mx, setup } from "./testing";

beforeEach(setup);

describe("git", () => {
  it("loads the diff of the history selection, ignores superseded answers and clears it", async () => {
    const c = (id: string, base: string) => ({ id, short: id, parents: [], base, subject: "s", author: "a", date: "", refs: [] });
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: false, commits: [c("c2", "c1"), c("c1", "e")] });
    mocked.impact.mockResolvedValue(imp());
    const late = { base: "c1", head: "c2", files: [], truncated: false };
    const fresh = { base: "e", head: "c1", files: [], truncated: false };
    let release: (v: typeof late) => void = () => {};
    mocked.diff.mockReturnValueOnce(new Promise((r) => (release = r))).mockResolvedValueOnce(fresh);
    await load("/p");
    expect(git.diff).toBeNull();
    pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    expect(mocked.diff).toHaveBeenLastCalledWith("/p", "c1", "c2");
    expect(git.diff).toEqual({ base: "c1", head: "c2", loading: true });
    await flush();
    runTypedImpact("e", "c1");
    await flush();
    release(late);
    await flush();
    expect(git.diff).toEqual({ base: "e", head: "c1", loading: false, data: fresh });
    clearImpact();
    expect(git.diff).toBeNull();
  });

  it("ignores a diff answer arriving after Clear, and loads the working tree diff", async () => {
    const c = (id: string, base: string) => ({ id, short: id, parents: [], base, subject: "s", author: "a", date: "", refs: [] });
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits: [c("c2", "c1")] });
    mocked.impact.mockResolvedValue(imp());
    let release: (v: { base: string; head: string; files: []; truncated: boolean }) => void = () => {};
    mocked.diff.mockReturnValueOnce(new Promise((r) => (release = r)));
    await load("/p");
    pickHistory({ kind: "range", newer: 0, older: 0, anchor: 0 });
    clearImpact();
    release({ base: "c1", head: "c2", files: [], truncated: false });
    await flush();
    expect(git.diff).toBeNull();
    mocked.diff.mockResolvedValue({ base: "HEAD", head: null, files: [], truncated: false });
    pickHistory({ kind: "worktree" });
    expect(mocked.diff).toHaveBeenLastCalledWith("/p", null, null);
  });

  it("reports a diff failure", async () => {
    await loaded();
    mocked.diff.mockRejectedValue(new Error("boom"));
    await git.loadDiff(null, null);
    expect(git.diff).toEqual({ base: null, head: null, loading: false, error: "Error: boom" });
  });

  it("selects the working tree and single commits", async () => {
    const c = (id: string, base: string) => ({ id, short: id, parents: [], base, subject: "s", author: "a", date: "", refs: [] });
    mocked.analyze.mockResolvedValue(mx("work"));
    mocked.history.mockResolvedValue({ dirty: true, commits: [c("c2", "c1"), c("c1", "e")] });
    mocked.impact.mockResolvedValue(imp());
    await load("/p");
    pickHistory({ kind: "worktree" });
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", null, null, expect.any(Function));
    await flush();
    pickHistory({ kind: "range", newer: 1, older: 1, anchor: 1 });
    expect(mocked.impact).toHaveBeenLastCalledWith("/p", "e", "c1", expect.any(Function));
  });
});

describe("git store on its own", () => {
  it("keeps a history only while its load is current", async () => {
    const g = new GitStore();
    mocked.history.mockResolvedValue({ dirty: false, commits: [] });
    await g.loadHistory("/p", () => false);
    expect(g.history).toBeNull();
    await g.loadHistory("/p", () => true);
    expect(g.history).not.toBeNull();
    g.reset();
    expect(g.history).toBeNull();
  });
});
