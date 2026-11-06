import { describe, expect, it } from "vitest";
import { CHANGED, DEFAULT_FILL, impactCellPaint, impactColor } from "./cellstate";

describe("impactColor", () => {
  it("steps through the token ramp with distance and saturates", () => {
    expect(impactColor(1)).toBe("var(--impact-1)");
    expect(impactColor(2)).toBe("var(--impact-2)");
    expect(impactColor(5)).toBe(impactColor(50));
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
