import { describe, expect, it } from "vitest";
import { clampWidth, DEFAULT_PANELS, MIN_MATRIX_WIDTH, MIN_WIDTH, parsePanels } from "./panels";

describe("clampWidth", () => {
  it("clamps to [MIN_WIDTH, 60% of the viewport]", () => {
    expect(clampWidth(10, 260, 1000)).toBe(MIN_WIDTH);
    expect(clampWidth(5000, 260, 1000)).toBe(600);
    expect(clampWidth(300.4, 260, 1000)).toBe(300);
  });
  it("uses MIN_WIDTH on a tiny viewport", () => {
    expect(clampWidth(500, 260, 100)).toBe(MIN_WIDTH);
  });
  it("uses the fallback for non-finite", () => {
    expect(clampWidth(NaN, 260, 1000)).toBe(260);
    expect(clampWidth(Infinity, 260, 1000)).toBe(260);
  });
  it("leaves the matrix a minimum width next to the other panel", () => {
    const vp = 1000;
    const w = clampWidth(5000, 260, vp, 340);
    expect(w).toBe(vp - 340 - 10 - MIN_MATRIX_WIDTH);
    expect(w + 340 + 10 + MIN_MATRIX_WIDTH).toBeLessThanOrEqual(vp);
  });
});

describe("parsePanels", () => {
  it("defaults on missing or invalid JSON", () => {
    expect(parsePanels(null)).toEqual(DEFAULT_PANELS);
    expect(parsePanels("{nope")).toEqual(DEFAULT_PANELS);
    expect(parsePanels("42")).toEqual(DEFAULT_PANELS);
    expect(parsePanels('{"left":"x","right":null}')).toEqual(DEFAULT_PANELS);
  });
  it("reads widths and collapsed flags, keeping the collapsed flag", () => {
    const s = parsePanels(
      '{"left":{"width":9999,"collapsed":true},"right":{"width":"a","collapsed":"yes"}}',
    );
    expect(s.left).toEqual({ width: 9999, collapsed: true });
    expect(s.right).toEqual({ width: DEFAULT_PANELS.right.width, collapsed: false });
  });
  it("round-trips a valid state", () => {
    const st = { left: { width: 200, collapsed: false }, right: { width: 400, collapsed: true } };
    expect(parsePanels(JSON.stringify(st))).toEqual(st);
  });
});
