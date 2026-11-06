import { describe, expect, it } from "vitest";
import { edgePath, fitText } from "./matrixgeom";

const tile = (x: number, y: number, w = 20) => ({ id: "t", layer: "l", domain: "d", x, y, w, h: w });

describe("edgePath", () => {
  it("starts and ends just outside the two tiles", () => {
    const p = edgePath(tile(0, 0), tile(100, 0));
    const nums = p.match(/-?\d+(\.\d+)?/g)?.map(Number) ?? [];
    expect(nums[0]).toBeCloseTo(10 + 10 + 2); // x1 + w/2 + 2
    expect(nums[1]).toBeCloseTo(10);
    expect(nums[4]).toBeCloseTo(110 - 12);
  });
  it("does not blow up for coincident tiles", () => {
    expect(edgePath(tile(0, 0), tile(0, 0))).not.toContain("NaN");
  });
});

describe("fitText", () => {
  it("keeps what fits and ellipsizes the rest", () => {
    expect(fitText("abc", 100, 8)).toBe("abc");
    expect(fitText("abcdefghij", 40, 8)).toBe("abcd…");
    expect(fitText("abcdefghij", 8, 8)).toBe("");
  });
});
