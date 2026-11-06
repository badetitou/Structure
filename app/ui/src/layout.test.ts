import { describe, expect, it } from "vitest";
import { DENSE_STEP, layout } from "./layout";

const layers = ["api", "service", "data"];
const domains = ["user", "order", "billing", "audit"];
const cells = [
  { id: "api:user", layer: "api", domain: "user", size: 10 },
  { id: "service:order", layer: "service", domain: "order", size: 40 },
];

describe("layout dense", () => {
  const l = layout("dense", layers, domains, cells);
  it("has one uniform tile per slot, empty ones with null id", () => {
    expect(l.tiles).toHaveLength(12);
    expect(l.tiles.filter((t) => t.id === null)).toHaveLength(10);
    for (const t of l.tiles) expect([t.w, t.h]).toEqual([DENSE_STEP, DENSE_STEP]);
  });
  it("packs tiles edge to edge without overlap", () => {
    const xs = [...new Set(l.tiles.map((t) => t.x))].sort((a, b) => a - b);
    for (let i = 1; i < xs.length; i++) expect(xs[i] - xs[i - 1]).toBe(DENSE_STEP);
    const ys = [...new Set(l.tiles.map((t) => t.y))].sort((a, b) => a - b);
    for (let i = 1; i < ys.length; i++) expect(ys[i] - ys[i - 1]).toBe(DENSE_STEP);
  });
  it("viewBox covers the grid", () => {
    for (const t of l.tiles) {
      expect(t.x + t.w).toBeLessThanOrEqual(l.width);
      expect(t.y + t.h).toBeLessThanOrEqual(l.height);
    }
    expect(l.rotateDomains).toBe(true);
    expect(l.detailed).toBe(false);
  });
});

describe("layout normal", () => {
  const l = layout("normal", layers, domains, cells);
  it("reproduces the 140px pitch geometry", () => {
    expect(l.width).toBe(110 + 4 * 140 + 20);
    expect(l.height).toBe(40 + 3 * 140 + 20);
    const empty = l.tiles.find((t) => t.layer === "api" && t.domain === "order");
    expect(empty).toMatchObject({ id: null, x: 110 + 140 + 20, y: 40 + 20, w: 100, h: 100 });
  });
  it("sizes the biggest cell at 20 + 90 and centres it", () => {
    const t = l.tiles.find((x) => x.id === "service:order");
    expect(t?.w).toBeCloseTo(110);
    expect((t?.x ?? 0) + (t?.w ?? 0) / 2).toBeCloseTo(110 + 140 + 70);
    expect(l.detailed).toBe(true);
    expect(l.rotateDomains).toBe(false);
  });
});
