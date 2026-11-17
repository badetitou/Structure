import { describe, expect, it } from "vitest";
import type { Impact } from "./api";
import {
  OVERLAY_COLOURS,
  addOverlay,
  clickSelection,
  isMacPlatform,
  isToggleClick,
  nextColour,
  sameSelection,
  selectionAfterRemoval,
  selectorVisible,
} from "./overlays";
import type { Overlay, Range } from "./overlays";

const imp = (base: string): Impact => ({
  base,
  head: null,
  changed_files: [],
  changed_symbols: [],
  changed_cells: [],
  impacted_cells: [],
  impacted_symbols: [],
});
const r = (base: string | null, head: string | null = null): Range => ({ base, head });
/** `list` with overlay `id` pinned. */
const pin = (list: Overlay[], id: string) => list.map((o) => (o.id === id ? { ...o, pinned: true } : o));

describe("overlay list", () => {
  it("labels a new overlay with its range and gives it the first colour", () => {
    const { list, id } = addOverlay([], r("b6a8d35".padEnd(40, "0"), "a94f1b7".padEnd(40, "1")), imp("x"), null, "o1");
    expect(id).toBe("o1");
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ id: "o1", label: "b6a8d35..a94f1b7", source: "git-diff", colour: "--overlay-0", pinned: false, stale: false });
    expect(addOverlay([], r(null), imp("x"), null, "o1").list[0].label).toBe("working tree");
  });

  it("replaces the unpinned overlay: one impact at a time unless kept", () => {
    const a = addOverlay([], r("a"), imp("a"), null, "o1");
    const b = addOverlay(a.list, r("b"), imp("b"), null, "o2");
    expect(b.list.map((o) => o.id)).toEqual(["o2"]);
    expect(b.list[0].colour).toBe("--overlay-0");
  });

  it("a pinned overlay stays; the new one is added after it with its own colour", () => {
    const a = addOverlay([], r("a"), imp("a"), null, "o1");
    const b = addOverlay(pin(a.list, "o1"), r("b"), imp("b"), null, "o2");
    expect(b.list.map((o) => o.id)).toEqual(["o1", "o2"]);
    expect(b.list.map((o) => o.colour)).toEqual(["--overlay-0", "--overlay-1"]);
    // the next one replaces o2, the unpinned one, and reuses the free colour
    const c = addOverlay(b.list, r("c"), imp("c"), null, "o3");
    expect(c.list.map((o) => o.id)).toEqual(["o1", "o3"]);
    expect(c.list[1].colour).toBe("--overlay-1");
  });

  it("the same range updates its overlay in place, keeping its id, colour, pin and history selection", () => {
    const sel = { kind: "worktree" } as const;
    const a = addOverlay([], r("a"), imp("a"), sel, "o1");
    const kept = pin(a.list, "o1").map((o) => ({ ...o, stale: true }));
    const b = addOverlay([...kept, { ...a.list[0], id: "o2", range: r("b"), colour: "--overlay-1" }], r("a"), imp("again"), null, "o3");
    expect(b.id).toBe("o1");
    expect(b.list.map((o) => o.id)).toEqual(["o1", "o2"]);
    expect(b.list[0]).toMatchObject({ pinned: true, stale: false, colour: "--overlay-0", historySel: sel });
    expect(b.list[0].impact.base).toBe("again");
  });

  it("colours cycle once all are taken", () => {
    let list: Overlay[] = [];
    for (let i = 0; i < OVERLAY_COLOURS; i++) list = pin(addOverlay(list, r(`r${i}`), imp("x"), null, `o${i}`).list, `o${i}`);
    expect(new Set(list.map((o) => o.colour)).size).toBe(OVERLAY_COLOURS);
    expect(nextColour(list)).toBe("--overlay-0");
  });

  it("selection after removal: kept when another overlay was shown, else the most recent left", () => {
    const left = [{ id: "o1" }, { id: "o3" }] as Overlay[];
    expect(selectionAfterRemoval(left, ["o1"], "o2")).toEqual(["o1"]);
    expect(selectionAfterRemoval(left, ["o2"], "o2")).toEqual(["o3"]);
    expect(selectionAfterRemoval(left, [], "o2")).toEqual([]);
    expect(selectionAfterRemoval([], ["o2"], "o2")).toEqual([]);
  });

  it("selection after removal: an overlay removed from several shown only leaves them", () => {
    const left = [{ id: "o1" }, { id: "o3" }] as Overlay[];
    expect(selectionAfterRemoval(left, ["o2", "o1"], "o2")).toEqual(["o1"]);
    expect(selectionAfterRemoval(left, ["o3", "o1"], "o2")).toEqual(["o3", "o1"]);
  });

  it("the selector is shown only with two overlays or more", () => {
    const one = addOverlay([], r("a"), imp("a"), null, "o1").list;
    expect(selectorVisible([], [])).toBe(false);
    expect(selectorVisible(one, ["o1"])).toBe(false);
    expect(selectorVisible(addOverlay(pin(one, "o1"), r("b"), imp("b"), null, "o2").list, ["o2"])).toBe(true);
  });

  it("the selector shows a single overlay left unselected so that it stays reachable", () => {
    const kept = pin(addOverlay([], r("a"), imp("a"), null, "o1").list, "o1");
    expect(selectorVisible(kept, [])).toBe(true);
    expect(selectorVisible(kept, ["o1"])).toBe(false);
  });
});

describe("overlay selection", () => {
  it("a plain click selects exactly that overlay", () => {
    expect(clickSelection([], "o1", false)).toEqual(["o1"]);
    expect(clickSelection(["o1", "o2"], "o3", false)).toEqual(["o3"]);
    expect(clickSelection(["o1", "o2"], "o2", false)).toEqual(["o2"]);
    // plain click on the single selected one keeps it
    expect(clickSelection(["o1"], "o1", false)).toEqual(["o1"]);
  });

  it("a toggle click adds or removes, in click order, but never empties the selection", () => {
    expect(clickSelection(["o2"], "o1", true)).toEqual(["o2", "o1"]);
    expect(clickSelection(["o2", "o1"], "o2", true)).toEqual(["o1"]);
    expect(clickSelection(["o1"], "o1", true)).toEqual(["o1"]);
    expect(clickSelection([], "o1", true)).toEqual(["o1"]);
  });

  it("compares selections by content and order", () => {
    expect(sameSelection(["a", "b"], ["a", "b"])).toBe(true);
    expect(sameSelection(["a", "b"], ["b", "a"])).toBe(false);
    expect(sameSelection(["a"], ["a", "b"])).toBe(false);
  });

  it("toggles with cmd on macOS, with ctrl elsewhere", () => {
    expect(isToggleClick({ metaKey: true, ctrlKey: false }, true)).toBe(true);
    expect(isToggleClick({ metaKey: false, ctrlKey: true }, true)).toBe(false);
    expect(isToggleClick({ metaKey: false, ctrlKey: true }, false)).toBe(true);
    expect(isToggleClick({ metaKey: true, ctrlKey: false }, false)).toBe(false);
    expect(isMacPlatform("MacIntel")).toBe(true);
    expect(isMacPlatform("Win32")).toBe(false);
    expect(isMacPlatform("Linux x86_64")).toBe(false);
  });
});
