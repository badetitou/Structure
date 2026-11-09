import { describe, expect, it } from "vitest";
import { CHANGED, DEFAULT_FILL, edgeKind, edgeStyle, defaultEdgeOpacity, hoverCellPaint, impactCellPaint, impactColor } from "./cellstate";
import { adjacency, reach } from "./graph";

describe("impactColor", () => {
  it("steps through the token ramp with distance and saturates", () => {
    expect(impactColor(1)).toBe("var(--impact-1)");
    expect(impactColor(2)).toBe("var(--impact-2)");
    expect(impactColor(5)).toBe("var(--impact-5)");
    expect(impactColor(50)).toBe("var(--impact-5)");
    expect(impactColor(0)).toBe("var(--impact-1)");
    expect(impactColor(Number.NaN)).toBe("var(--impact-5)");
    expect(impactColor(Number.POSITIVE_INFINITY)).toBe("var(--impact-5)");
  });
});

describe("impactCellPaint", () => {
  const changed = new Set(["a"]);
  const impacted = new Map([
    ["a", 1],
    ["b", 1],
  ]);
  it("changed beats impacted", () => {
    expect(impactCellPaint("a", changed, impacted)).toEqual({ fill: CHANGED, opacity: 1 });
  });
  it("impacted beats dimmed", () => {
    const p = impactCellPaint("b", changed, impacted);
    expect(p.fill).toBe(impactColor(1));
    expect(p.opacity).toBe(1);
  });
  it("others are dimmed", () => {
    expect(impactCellPaint("z", changed, impacted)).toEqual({ fill: DEFAULT_FILL, opacity: 0.25 });
  });
});

const e = (from: string, to: string) => ({ from, to, weight: 1 });
const hoverOf = (id: string, edges: ReturnType<typeof e>[]) => {
  const adj = adjacency(edges);
  return { id, fwd: reach(id, adj, "forward"), bwd: reach(id, adj, "backward") };
};

describe("hover paint", () => {
  const h = hoverOf("b", [e("a", "b"), e("b", "c"), e("c", "b"), e("x", "y")]);
  it("colours the hovered cell, its reach, and dims the rest", () => {
    expect(hoverCellPaint("b", h).fill).toBe("var(--hover)");
    expect(hoverCellPaint("c", h).fill).toBe("var(--both)");
    expect(hoverCellPaint("a", h).fill).toBe("var(--bwd)");
    expect(hoverCellPaint("x", h)).toEqual({ fill: DEFAULT_FILL, opacity: 0.25 });
  });
  it("kinds the edges by what the hovered cell reaches", () => {
    expect(edgeKind("x", "y", h, null)).toBe("def");
    expect(edgeKind("a", "b", h, null)).toBe("bwd");
    expect(edgeKind("b", "c", h, null)).toBe("fwd");
  });
  it("edges fall back to impact then default", () => {
    const imp = { changed: new Set(["a"]), impacted: new Map([["b", 1]]) };
    expect(edgeKind("a", "b", null, imp)).toBe("imp");
    expect(edgeKind("b", "c", null, imp)).toBe("def");
    expect(edgeKind("a", "b", null, null)).toBe("def");
  });
});

// Values the imperative matrixview.ts used for edges (main:app/ui/src/matrixview.ts, paint()).
describe("edge style parity", () => {
  it("matches the old colour / marker / opacity per state", () => {
    expect(edgeStyle("fwd", 1)).toMatchObject({ stroke: "var(--fwd)", marker: "a-fwd", opacity: "0.95" });
    expect(edgeStyle("def", 1, "rpc")).toMatchObject({ stroke: "var(--edge-rpc)", marker: "a-rpc" });
    expect(edgeStyle("def", 1, "http")).toMatchObject({ stroke: "var(--edge-http)", marker: "a-http" });
    expect(edgeStyle("fwd", 1, "http")).toMatchObject({ stroke: "var(--fwd)" });
    expect(edgeStyle("bwd", 1)).toMatchObject({ stroke: "var(--bwd)", marker: "a-bwd", opacity: "0.95" });
    expect(edgeStyle("imp", 1)).toMatchObject({ stroke: "var(--changed)", marker: "a-imp", opacity: "0.9" });
    expect(edgeStyle("def", 1)).toMatchObject({ stroke: "var(--edge)", marker: "a-def" });
  });
  it("default edges are 0.5, 0.1 while hovering, 0.12 while showing impact", () => {
    expect(defaultEdgeOpacity(false, false)).toBe(0.5);
    expect(defaultEdgeOpacity(true, true)).toBe(0.1);
    expect(defaultEdgeOpacity(false, true)).toBe(0.12);
  });
  it("width is max(1, 1 + ln weight)", () => {
    expect(edgeStyle("def", 1).width).toBe(1);
    expect(edgeStyle("def", 10).width).toBeCloseTo(1 + Math.log(10));
  });
});
