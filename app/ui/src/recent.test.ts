import { describe, expect, it } from "vitest";
import { MAX_RECENT, type RecentProject, addRecent, analyzeAndRecord, folderName, parseRecent, removeRecent } from "./recent";

const d = (n: number) => new Date(Date.UTC(2026, 0, n));

describe("recent", () => {
  it("adds most recent first with folder name", () => {
    const l = addRecent(addRecent([], "/a/one", d(1)), "/a/two/", d(2));
    expect(l.map((r) => r.name)).toEqual(["two", "one"]);
    expect(l[0].lastOpened).toBe(d(2).toISOString());
  });
  it("dedupes and moves to top", () => {
    let l = addRecent([], "/a", d(1));
    l = addRecent(l, "/b", d(2));
    l = addRecent(l, "/a", d(3));
    expect(l.map((r) => r.path)).toEqual(["/a", "/b"]);
    expect(l[0].lastOpened).toBe(d(3).toISOString());
  });
  it("caps the list", () => {
    let l = addRecent([], "/p0", d(1));
    for (let i = 1; i < 12; i++) l = addRecent(l, `/p${i}`, d(1));
    expect(l).toHaveLength(MAX_RECENT);
    expect(l[0].path).toBe("/p11");
  });
  it("removes", () => {
    const l = addRecent(addRecent([], "/a", d(1)), "/b", d(2));
    expect(removeRecent(l, "/a").map((r) => r.path)).toEqual(["/b"]);
  });
  it("parses corrupt data to empty", () => {
    expect(parseRecent(null)).toEqual([]);
    expect(parseRecent("{nope")).toEqual([]);
    expect(parseRecent('{"a":1}')).toEqual([]);
    expect(parseRecent('[{"path":1},null]')).toEqual([]);
  });
  it("round-trips valid data", () => {
    const l = addRecent([], "/a", d(1));
    expect(parseRecent(JSON.stringify(l))).toEqual(l);
  });
  it("folderName handles windows paths", () => {
    expect(folderName("C:\\x\\proj")).toBe("proj");
  });
});

describe("analyzeAndRecord", () => {
  const memStore = (init: RecentProject[]) => {
    let data = init;
    return {
      load: () => data,
      save: (l: RecentProject[]) => {
        data = l;
      },
      get: () => data,
    };
  };
  it("records the path at the top on success", async () => {
    const store = memStore(addRecent([], "/old", d(1)));
    const r = await analyzeAndRecord("/new", async () => 42, store, d(2));
    expect(r).toBe(42);
    expect(store.get().map((x) => x.path)).toEqual(["/new", "/old"]);
  });
  it("leaves the store untouched and rethrows on failure", async () => {
    const initial = addRecent([], "/old", d(1));
    const store = memStore(initial);
    await expect(
      analyzeAndRecord("/bad", () => Promise.reject(new Error("boom")), store, d(2)),
    ).rejects.toThrow("boom");
    expect(store.get()).toBe(initial);
    expect(store.get().map((x) => x.path)).toEqual(["/old"]);
  });
});
