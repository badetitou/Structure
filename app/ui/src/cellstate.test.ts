import { describe, expect, it } from "vitest";
import { CHANGED, DEFAULT_FILL, impactCellPaint, impactColor } from "./cellstate";

describe("impactColor", () => {
  const lightness = (s: string) => Number(/(\d+(?:\.\d+)?)%\)$/.exec(s)?.[1]);
  it("gets lighter with distance and saturates", () => {
    expect(lightness(impactColor(1))).toBeLessThan(lightness(impactColor(2)));
    expect(lightness(impactColor(5))).toBe(lightness(impactColor(50)));
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
