import { afterEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_LAYOUT,
  LAYOUT_KEY,
  dragFraction,
  leaf,
  loadLayout,
  normalize,
  normalizeSizes,
  panelsOf,
  parseLayout,
  resizePair,
  saveLayout,
  split,
  visible,
} from "./tiling";
import type { Node, Split } from "./tiling";

const all = () => true;
const known = (ids: string[]) => (p: string) => ids.includes(p);
const close = (a: number[], b: number[]) => a.forEach((x, i) => expect(x).toBeCloseTo(b[i]));

describe("normalizeSizes", () => {
  it("scales to 1", () => close(normalizeSizes([1, 3]), [0.25, 0.75]));
  it("gives bad entries the average share", () => close(normalizeSizes([1, NaN, -2, 1]), [0.25, 0.25, 0.25, 0.25]));
  it("splits equally when nothing is valid", () => close(normalizeSizes([0, 0]), [0.5, 0.5]));
});

describe("normalize", () => {
  it("drops empty leaves and unwraps single-child splits", () => {
    const n = normalize(split("s", "row", [leaf("a", []), leaf("b", ["x"])]));
    expect(n).toEqual(leaf("b", ["x"]));
  });
  it("returns null when nothing is left", () => {
    expect(normalize(split("s", "row", [leaf("a", [])]))).toBeNull();
  });
  it("flattens a split nested in one of the same direction, keeping proportions", () => {
    const n = normalize(
      split("s", "row", [leaf("a", ["a"]), split("t", "row", [leaf("b", ["b"]), leaf("c", ["c"])], [0.5, 0.5])], [0.5, 0.5]),
    ) as Split;
    expect(n.children.map((c) => c.id)).toEqual(["a", "b", "c"]);
    close(n.sizes, [0.5, 0.25, 0.25]);
  });
  it("keeps a split of the other direction", () => {
    const n = normalize(split("s", "row", [leaf("a", ["a"]), split("t", "column", [leaf("b", ["b"]), leaf("c", ["c"])])])) as Split;
    expect(n.children[1].kind).toBe("split");
  });
  it("fixes an invalid active panel and duplicate panels", () => {
    expect(normalize(leaf("a", ["x", "y", "x"], "z"))).toEqual(leaf("a", ["x", "y"], "x"));
  });
});

describe("visible", () => {
  it("hides unavailable panels and gives their room to the others", () => {
    const v = visible(DEFAULT_LAYOUT, (p) => p !== "history") as Split;
    expect(v.children.map((c) => c.id)).toEqual(["t-matrix", "t-details"]);
    close(v.sizes, [0.55 / 0.8, 0.25 / 0.8]);
  });
  it("moves the active panel off an unavailable one", () => {
    expect(visible(leaf("a", ["x", "y"], "x"), (p) => p === "y")).toEqual(leaf("a", ["y"], "y"));
  });
  it("is null when nothing is available", () => {
    expect(visible(DEFAULT_LAYOUT, () => false)).toBeNull();
  });
});

describe("resizePair", () => {
  it("shares the pair's size and leaves the others", () => {
    const r = resizePair(DEFAULT_LAYOUT, "root", "t-matrix", "t-details", 0.5) as Split;
    close(r.sizes, [0.2, 0.4, 0.4]);
  });
  it("applies to the stored tree a resize made on the shown one", () => {
    const shown = visible(DEFAULT_LAYOUT, (p) => p !== "history") as Split;
    expect(shown.id).toBe("root");
    const r = resizePair(DEFAULT_LAYOUT, shown.id, "t-matrix", "t-details", 0.75) as Split;
    close(r.sizes, [0.2, 0.6, 0.2]);
  });
  it("reaches nested splits", () => {
    const t = split("s", "row", [leaf("a", ["a"]), split("t", "column", [leaf("b", ["b"]), leaf("c", ["c"])])]);
    const r = resizePair(t, "t", "b", "c", 0.2) as Split;
    close((r.children[1] as Split).sizes, [0.2, 0.8]);
    expect(r.children[0]).toBe(t.children[0]);
  });
  it("ignores unknown ids and non-finite fractions", () => {
    expect(resizePair(DEFAULT_LAYOUT, "nope", "t-matrix", "t-details", 0.5)).toBe(DEFAULT_LAYOUT);
    expect(resizePair(DEFAULT_LAYOUT, "root", "t-matrix", "x", 0.5)).toBe(DEFAULT_LAYOUT);
    expect(resizePair(DEFAULT_LAYOUT, "root", "t-matrix", "t-details", NaN)).toBe(DEFAULT_LAYOUT);
  });
});

describe("dragFraction", () => {
  it("follows the pointer", () => expect(dragFraction(300, 300, 60, 100)).toBeCloseTo(0.6));
  it("keeps each side at least `min`", () => {
    expect(dragFraction(300, 300, 1000, 100)).toBeCloseTo(500 / 600);
    expect(dragFraction(300, 300, -1000, 100)).toBeCloseTo(100 / 600);
  });
  it("splits evenly when the pair is smaller than two minimums", () => {
    expect(dragFraction(50, 50, 30, 100)).toBeCloseTo(0.5);
  });
});

describe("parseLayout", () => {
  const ids = known(["matrix", "details", "history"]);
  it("falls back on missing, invalid or malformed JSON", () => {
    expect(parseLayout(null, ids)).toBe(DEFAULT_LAYOUT);
    expect(parseLayout("{nope", ids)).toBe(DEFAULT_LAYOUT);
    expect(parseLayout("42", ids)).toBe(DEFAULT_LAYOUT);
    expect(parseLayout('{"kind":"split","id":"s","dir":"diagonal","children":[]}', ids)).toBe(DEFAULT_LAYOUT);
    expect(parseLayout('{"kind":"leaf","panels":["matrix"]}', ids)).toBe(DEFAULT_LAYOUT);
  });
  it("rejects duplicate node ids", () => {
    const t = split("s", "row", [leaf("a", ["matrix"]), leaf("a", ["details"])]);
    expect(parseLayout(JSON.stringify(t), ids)).toBe(DEFAULT_LAYOUT);
  });
  it("round-trips a valid layout", () => {
    const t: Node = split("s", "column", [leaf("a", ["matrix"]), split("t", "row", [leaf("b", ["details"]), leaf("c", ["history"])], [0.3, 0.7])], [0.6, 0.4]);
    expect(parseLayout(JSON.stringify(t), ids)).toEqual(t);
  });
  it("drops unknown panels, and falls back when none are left", () => {
    const t = split("s", "row", [leaf("a", ["matrix"]), leaf("b", ["gone"])], [0.5, 0.5]);
    expect(parseLayout(JSON.stringify(t), ids)).toEqual(leaf("a", ["matrix"]));
    expect(parseLayout(JSON.stringify(leaf("a", ["gone"])), ids)).toBe(DEFAULT_LAYOUT);
  });
  it("repairs missing sizes", () => {
    const raw = '{"kind":"split","id":"s","dir":"row","children":[{"kind":"leaf","id":"a","panels":["matrix"]},{"kind":"leaf","id":"b","panels":["details"]}]}';
    close((parseLayout(raw, ids) as Split).sizes, [0.5, 0.5]);
  });
  it("the default places every built-in panel once", () => {
    expect(panelsOf(DEFAULT_LAYOUT).sort()).toEqual(["details", "history", "matrix"]);
    expect(normalize(DEFAULT_LAYOUT)).toEqual(DEFAULT_LAYOUT);
    expect(visible(DEFAULT_LAYOUT, all)).toEqual(DEFAULT_LAYOUT);
  });
});

describe("layout persistence", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("round-trips through storage", () => {
    const m = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) });
    const t = resizePair(DEFAULT_LAYOUT, "root", "t-history", "t-matrix", 0.5);
    saveLayout(t);
    expect(m.has(LAYOUT_KEY)).toBe(true);
    expect(loadLayout(all)).toEqual(t);
  });
  it("falls back to the default when storage throws", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    });
    expect(() => saveLayout(DEFAULT_LAYOUT)).not.toThrow();
    expect(loadLayout(all)).toBe(DEFAULT_LAYOUT);
  });
});
