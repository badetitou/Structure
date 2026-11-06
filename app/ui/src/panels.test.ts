import { describe, expect, it } from "vitest";
import { clampWidth, DEFAULT_PANELS, MAX_WIDTH, MIN_WIDTH, parsePanels } from "./panels";

describe("clampWidth", () => {
  it("clamps to the range", () => {
    expect(clampWidth(10, 260)).toBe(MIN_WIDTH);
    expect(clampWidth(5000, 260)).toBe(MAX_WIDTH);
    expect(clampWidth(300.4, 260)).toBe(300);
  });
  it("uses the fallback for non-finite", () => {
    expect(clampWidth(NaN, 260)).toBe(260);
    expect(clampWidth(Infinity, 260)).toBe(260);
  });
});

describe("parsePanels", () => {
  it("defaults on missing or invalid JSON", () => {
    expect(parsePanels(null)).toEqual(DEFAULT_PANELS);
    expect(parsePanels("{nope")).toEqual(DEFAULT_PANELS);
    expect(parsePanels("42")).toEqual(DEFAULT_PANELS);
    expect(parsePanels('{"left":"x","right":null}')).toEqual(DEFAULT_PANELS);
  });
  it("reads widths and collapsed flags, clamping widths", () => {
    const s = parsePanels(
      '{"left":{"width":9999,"collapsed":true},"right":{"width":"a","collapsed":"yes"}}',
    );
    expect(s.left).toEqual({ width: MAX_WIDTH, collapsed: true });
    expect(s.right).toEqual({ width: DEFAULT_PANELS.right.width, collapsed: false });
  });
  it("round-trips a valid state", () => {
    const st = { left: { width: 200, collapsed: false }, right: { width: 400, collapsed: true } };
    expect(parsePanels(JSON.stringify(st))).toEqual(st);
  });
});
